use axum::{
    async_trait,
    extract::Extension,
    extract::{FromRequestParts, Json},
    http::{request::Parts, StatusCode},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use chrono::{Duration, Utc};
use rusqlite::Connection;
use prometheus::IntCounterVec;
use std::sync::LazyLock;

static AUTH_REFRESH_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        prometheus::Opts::new("aegis_auth_refresh_total", "Refresh endpoint outcomes"),
        &["outcome"],
    )
    .expect("metric");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub jti: Option<String>,        // JWT ID for revocation
    pub scope: Vec<String>,
    pub iss: String,
    pub token_type: String, // "access" | "refresh"
}

#[derive(Clone)]
pub struct AuthState {
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
    pub validation: Validation,
    pub access_lifetime: Duration,
    pub refresh_lifetime: Duration,
    pub auth_db_path: String,
    /// In-memory revocation list for compromised access tokens (jti).
    /// В Enterprise-версии заменить на Redis/SQLite + TTL.
    pub revoked_access_jti: Arc<std::sync::Mutex<HashSet<String>>>,
    /// In-memory revocation list for refresh tokens (jti) + их exp (для GC/TTL).
    /// В Enterprise-версии заменить на SQLite/Redis (revoked_at + expires_at).
    pub revoked_refresh_jti: Arc<std::sync::Mutex<HashMap<String, i64>>>,
    /// Pepper for API key hashing (JWT secret bytes).
    pub api_key_pepper: Vec<u8>,
}

pub struct AuthUser(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> std::result::Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, _state)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

        let state = parts.extensions.get::<Arc<AuthState>>()
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Auth not configured"))?;

        let decoded = decode::<Claims>(bearer.token(), &state.decoding_key, &state.validation)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token"))?;

        if decoded.claims.token_type != "access" {
            return Err((StatusCode::UNAUTHORIZED, "Use access token"));
        }

        // === ZERO-TRUST: Token Revocation Check ===
        if let Some(jti) = &decoded.claims.jti {
            let revoked = state.revoked_access_jti.lock().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Lock poisoned"))?;
            if revoked.contains(jti) {
                return Err((StatusCode::UNAUTHORIZED, "Token has been revoked"));
            }
        }

        Ok(AuthUser(decoded.claims))
    }
}

// ==================== Создание токенов ====================
pub fn create_access_token(sub: &str, scopes: Vec<String>, state: &AuthState) -> std::result::Result<String, String> {
    let now = Utc::now();
    let jti = uuid::Uuid::new_v4().to_string();
    let claims = Claims {
        sub: sub.to_string(),
        iat: now.timestamp() as usize,
        exp: (now + state.access_lifetime).timestamp() as usize,
        jti: Some(jti),
        scope: scopes,
        iss: "aegis.oracle".to_string(),
        token_type: "access".to_string(),
    };
    encode(&Header::new(Algorithm::HS256), &claims, &state.encoding_key)
        .map_err(|e| e.to_string())
}

pub fn create_refresh_token(sub: &str, state: &AuthState) -> std::result::Result<String, String> {
    let now = Utc::now();
    let jti = uuid::Uuid::new_v4().to_string();
    let claims = Claims {
        sub: sub.to_string(),
        iat: now.timestamp() as usize,
        exp: (now + state.refresh_lifetime).timestamp() as usize,
        jti: Some(jti),
        scope: vec!["refresh".to_string()],
        iss: "aegis.oracle".to_string(),
        token_type: "refresh".to_string(),
    };
    encode(&Header::new(Algorithm::HS256), &claims, &state.encoding_key)
        .map_err(|e| e.to_string())
}

// ==================== Инициализация ====================
pub fn init_auth_state(secret: &[u8]) -> Arc<AuthState> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = 30;

    let auth_db_path = std::env::var("AEGIS_AUTH_DB").unwrap_or_else(|_| "aegis_auth.db".to_string());
    // Init schema synchronously at startup (fast); runtime operations use spawn_blocking.
    if let Ok(conn) = Connection::open(&auth_db_path) {
        let _ = conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS refresh_revocations (
                jti TEXT PRIMARY KEY,
                sub TEXT,
                revoked_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_refresh_revocations_expires_at ON refresh_revocations(expires_at);
            ",
        );
        let _ = crate::api_key_store::init_schema(&conn);
        if let Err(e) = crate::api_key_store::migrate_env_keys_once(&auth_db_path, secret) {
            tracing::warn!("api_key_store env migrate: {}", e);
        }
    }

    Arc::new(AuthState {
        encoding_key: EncodingKey::from_secret(secret),
        decoding_key: DecodingKey::from_secret(secret),
        validation,
        access_lifetime: Duration::minutes(15),
        // Refresh TTL: 14 дней (можно уменьшить до 7 в более строгих профилях)
        refresh_lifetime: Duration::days(14),
        auth_db_path,
        revoked_access_jti: Arc::new(std::sync::Mutex::new(HashSet::new())),
        revoked_refresh_jti: Arc::new(std::sync::Mutex::new(HashMap::new())),
        api_key_pepper: secret.to_vec(),
    })
}

// ==================== Login (совместимость с server.rs) ====================

#[derive(Deserialize)]
pub struct LoginRequest {
    pub api_key: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix timestamp (seconds) когда refresh истечёт (для UI/аудита)
    pub refresh_expires_at: i64,
    pub tier: String,
    pub expires_in: u64,
}

pub async fn login(
    Extension(state): Extension<Arc<AuthState>>,
    Json(payload): Json<LoginRequest>
) -> Result<Json<LoginResponse>, StatusCode> {
    // ZERO-TRUST: test keys only when AEGIS_DEV_MODE=1; production keys from env (chmod 600 agent.env).
    let is_dev = std::env::var("AEGIS_DEV_MODE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let identity = {
        let db_path = state.auth_db_path.clone();
        let key = payload.api_key.clone();
        let pepper = state.api_key_pepper.clone();
        tokio::task::spawn_blocking(move || {
            crate::api_key_store::lookup_identity(&db_path, &key, &pepper)
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let (uid, tier, scopes) = if let Some(id) = identity {
        (id.sub, id.tier, id.scopes)
    } else {
        match payload.api_key.as_str() {
            "test-key-starter" if is_dev => (
                "user-1".to_string(),
                "starter".to_string(),
                vec!["starter".to_string(), "read".to_string(), "threats".to_string()],
            ),
            "test-key-pro" if is_dev => (
                "user-2".to_string(),
                "pro".to_string(),
                vec!["pro".to_string(), "read".to_string(), "threats".to_string()],
            ),
            "test-key-enterprise" if is_dev => (
                "user-3".to_string(),
                "enterprise".to_string(),
                vec![
                    "enterprise".to_string(),
                    "read".to_string(),
                    "threats".to_string(),
                ],
            ),
            _ => {
                tracing::warn!("[SECURITY] Login rejected (dev_mode={})", is_dev);
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    };

    let access_token = create_access_token(
        &uid,
        scopes,
        &state,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let refresh_token = create_refresh_token(&uid, &state)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let refresh_expires_at = (Utc::now() + state.refresh_lifetime).timestamp();

    tracing::info!("[AUTH] Issued access+refresh for {} (tier: {})", uid, tier);

    Ok(Json(LoginResponse {
        access_token,
        refresh_token,
        refresh_expires_at,
        tier: tier.to_string(),
        expires_in: 15 * 60,
    }))
}

/// Refresh Token endpoint (Фаза 5)
#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub expires_in: u64,
}

pub async fn refresh(
    Extension(state): Extension<Arc<AuthState>>,
    audit: Option<Extension<Arc<crate::audit::AuditTrail>>>,
    Json(payload): Json<RefreshRequest>
) -> Result<Json<RefreshResponse>, StatusCode> {
    let decoded = match decode::<Claims>(&payload.refresh_token, &state.decoding_key, &state.validation) {
        Ok(d) => d,
        Err(_) => {
            AUTH_REFRESH_TOTAL.with_label_values(&["decode_fail"]).inc();
            if let Some(Extension(audit)) = audit {
                let _ = audit.log_event("api", "refresh_denied_decode_fail", 0.4, false);
            }
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    if decoded.claims.token_type != "refresh" {
        AUTH_REFRESH_TOTAL.with_label_values(&["wrong_type"]).inc();
        if let Some(Extension(audit)) = audit {
            let _ = audit.log_event("api", "refresh_denied_wrong_type", 0.4, false);
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Refresh revocation check (jti) — in-memory (fast path) + SQLite (durable)
    if let Some(jti) = &decoded.claims.jti {
        let revoked = state.revoked_refresh_jti.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if revoked.contains_key(jti) {
            AUTH_REFRESH_TOTAL.with_label_values(&["revoked_mem"]).inc();
            if let Some(Extension(audit)) = audit {
                let _ = audit.log_event("api", &format!("refresh_denied_revoked_mem sub={}", decoded.claims.sub), 0.6, false);
            }
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    if let Some(jti) = decoded.claims.jti.as_deref() {
        if is_refresh_revoked_sqlite(&state.auth_db_path, jti).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
            AUTH_REFRESH_TOTAL.with_label_values(&["revoked_sqlite"]).inc();
            if let Some(Extension(audit)) = audit {
                let _ = audit.log_event("api", &format!("refresh_denied_revoked_sqlite sub={}", decoded.claims.sub), 0.6, false);
            }
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    let now = Utc::now();
    let new_access = Claims {
        sub: decoded.claims.sub.clone(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::minutes(15)).timestamp() as usize,
        jti: Some(uuid::Uuid::new_v4().to_string()),
        scope: vec!["read".to_string(), "threats".to_string()],
        iss: "aegis.oracle".to_string(),
        token_type: "access".to_string(),
    };

    let token = encode(&Header::new(Algorithm::HS256), &new_access, &state.encoding_key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    AUTH_REFRESH_TOTAL.with_label_values(&["ok"]).inc();
    if let Some(Extension(audit)) = audit {
        let _ = audit.log_event("api", &format!("refresh_ok sub={}", decoded.claims.sub), 0.1, true);
    }

    Ok(Json(RefreshResponse {
        access_token: token,
        expires_in: 15 * 60,
    }))
}

/// Revoke a token by its jti (Zero-Trust revocation)
/// Call this on logout or when compromise is detected.
pub fn revoke_token(state: &AuthState, jti: &str) {
    if let Ok(mut revoked) = state.revoked_access_jti.lock() {
        revoked.insert(jti.to_string());
    }
}

/// Revoke refresh token by its jti. Stores exp for TTL/GC.
pub fn revoke_refresh_token(state: &AuthState, jti: &str, expires_at: i64) {
    if let Ok(mut revoked) = state.revoked_refresh_jti.lock() {
        revoked.insert(jti.to_string(), expires_at);
    }
    let db_path = state.auth_db_path.clone();
    let jti_s = jti.to_string();
    tokio::spawn(async move {
        let _ = store_refresh_revocation_sqlite(&db_path, &jti_s, None, expires_at).await;
    });
}

pub fn start_refresh_revocation_gc(state: Arc<AuthState>) {
    let period_secs: u64 = std::env::var("AEGIS_AUTH_GC_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);
    if period_secs == 0 {
        return;
    }
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(period_secs));
        loop {
            ticker.tick().await;
            let db_path = state.auth_db_path.clone();
            let _ = tokio::task::spawn_blocking(move || {
                if let Ok(conn) = Connection::open(&db_path) {
                    let _ = conn.execute(
                        "DELETE FROM refresh_revocations WHERE expires_at < ?1",
                        (Utc::now().timestamp(),),
                    );
                }
            })
            .await;

            if let Ok(mut mem) = state.revoked_refresh_jti.lock() {
                let now = Utc::now().timestamp();
                mem.retain(|_, exp| *exp >= now);
            }
        }
    });
}

async fn store_refresh_revocation_sqlite(
    db_path: &str,
    jti: &str,
    sub: Option<&str>,
    expires_at: i64,
) -> Result<(), String> {
    let db_path = db_path.to_string();
    let jti = jti.to_string();
    let sub = sub.map(|s| s.to_string());
    let revoked_at = Utc::now().timestamp();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let conn = Connection::open(&db_path).map_err(|e| format!("auth db open error: {}", e))?;
        // GC: purge expired
        let _ = conn.execute("DELETE FROM refresh_revocations WHERE expires_at < ?1", (Utc::now().timestamp(),));
        conn.execute(
            "INSERT OR REPLACE INTO refresh_revocations (jti, sub, revoked_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
            (&jti, sub.as_deref(), revoked_at, expires_at),
        )
        .map_err(|e| format!("auth db insert error: {}", e))?;
        Ok(())
    })
    .await
    .map_err(|e| format!("auth db join error: {}", e))?
}

async fn is_refresh_revoked_sqlite(db_path: &str, jti: &str) -> Result<bool, String> {
    let db_path = db_path.to_string();
    let jti = jti.to_string();
    tokio::task::spawn_blocking(move || -> Result<bool, String> {
        let conn = Connection::open(&db_path).map_err(|e| format!("auth db open error: {}", e))?;
        let now = Utc::now().timestamp();
        let _ = conn.execute("DELETE FROM refresh_revocations WHERE expires_at < ?1", (now,));
        let mut stmt = conn
            .prepare("SELECT 1 FROM refresh_revocations WHERE jti = ?1 AND expires_at >= ?2 LIMIT 1")
            .map_err(|e| format!("auth db prepare error: {}", e))?;
        let mut rows = stmt.query((&jti, now)).map_err(|e| format!("auth db query error: {}", e))?;
        Ok(rows.next().map_err(|e| format!("auth db row error: {}", e))?.is_some())
    })
    .await
    .map_err(|e| format!("auth db join error: {}", e))?
}