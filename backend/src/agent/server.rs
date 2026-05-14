use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::ConnectInfo,
    extract::State,
    http::{HeaderName, HeaderValue},
    http::{Request, StatusCode},
    response::IntoResponse,
    middleware::{from_fn_with_state, Next},
    routing::get,
    Json, Router,
};
use tower_http::cors::{Any, CorsLayer};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::auth::login;
use crate::persistence::PersistentStore;
use crate::event_bus::EventBus;
use crate::fusion_engine::FusionEngine;
use std::fs;
use std::path::Path;
use prometheus::{Encoder, TextEncoder};

#[derive(Debug, Clone, Copy)]
struct RateLimitRule {
    name: &'static str,
    max_requests: usize,
    window: std::time::Duration,
}

#[derive(Clone)]
struct RateLimiter {
    inner: Arc<Mutex<HashMap<String, VecDeque<std::time::Instant>>>>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn allow(&self, key: &str, rule: RateLimitRule) -> bool {
        let now = std::time::Instant::now();
        let mut map = self.inner.lock().await;
        let q = map.entry(key.to_string()).or_insert_with(VecDeque::new);

        while let Some(front) = q.front().copied() {
            if now.duration_since(front) > rule.window {
                q.pop_front();
            } else {
                break;
            }
        }

        if q.len() >= rule.max_requests {
            return false;
        }

        q.push_back(now);
        true
    }
}

#[derive(Clone)]
struct RateLimitState {
    limiter: Arc<RateLimiter>,
    rule: RateLimitRule,
}

async fn rate_limit_middleware(
    State(st): State<RateLimitState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Keyed by client IP + rule name (endpoint)
    let key = format!("ip:{}:{}", ip, st.rule.name);
    if !st.limiter.allow(&key, st.rule).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "rate_limited",
                "message": "Too many requests. Slow down."
            })),
        )
            .into_response();
    }

    next.run(req).await
}

#[derive(Clone)]
pub struct AppState {
    pub alert_tx: tokio::sync::broadcast::Sender<String>,
    pub store: PersistentStore,
    pub event_bus: EventBus,
    pub oracle_alive: Arc<std::sync::Mutex<bool>>,
    pub pending_acks: Arc<Mutex<HashMap<String, (String, std::time::Instant)>>>,
    pub fusion: Option<Arc<FusionEngine>>,
    pub auth: Option<Arc<crate::auth::AuthState>>,
    pub audit: Option<Arc<crate::audit::AuditTrail>>,
    pub federation: Option<Arc<crate::federation::FederationLayer>>,
    rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(store: PersistentStore) -> Self {
        // Increased buffer for high-load Enterprise scenarios (DDoS, mass scanning)
let (alert_tx, _) = tokio::sync::broadcast::channel(1024);
        Self {
            alert_tx,
            store,
            event_bus: EventBus::new(1000),
            oracle_alive: Arc::new(std::sync::Mutex::new(true)),
            pending_acks: Arc::new(Mutex::new(HashMap::new())),
            fusion: None,
            auth: None,
            audit: None,
            federation: None,
            rate_limiter: Arc::new(RateLimiter::new()),
        }
    }

    pub fn with_fusion(mut self, fusion: Arc<FusionEngine>) -> Self {
        self.fusion = Some(fusion);
        self
    }

    pub fn with_auth(mut self, auth: Arc<crate::auth::AuthState>) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn with_audit(mut self, audit: Arc<crate::audit::AuditTrail>) -> Self {
        self.audit = Some(audit);
        self
    }

    pub fn with_federation(mut self, federation: Arc<crate::federation::FederationLayer>) -> Self {
        self.federation = Some(federation);
        self
    }
}

#[derive(Serialize)]
struct StatusResponse {
    oracle_alive: bool,
    active_sentinels: u64,
    threats_blocked: u64,
    osint_documents: u64,
    darknet_documents: u64,
    shield_active: bool,
    version: String,
    air_gapped: bool,
}

#[derive(Serialize)]
struct KnowledgeResponse {
    osint: Vec<String>,
    darknet: Vec<String>,
}

#[derive(Serialize)]
struct ThreatResponse {
    id: String,
    message: String,
    severity: f64,
    timestamp: String,
    blocked: bool,
}

#[derive(Serialize)]
struct AgentStatusResponse {
    id: String,
    role: String,
    status: String,
    load: u64,
    critic: f64,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct PilotRequest {
    name: String,
    company: String,
    email: String,
    use_case: String,
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "oracle": "alive", "version": "8.0.0"}))
}

async fn api_status(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,   // JWT required
) -> Json<StatusResponse> {
    let oracle = *state.oracle_alive.lock().unwrap();
    let sentinels = state.store.get("active_sentinels").await;
    let blocked = state.store.get("threats_blocked").await;
    let osint = state.store.get("osint_count").await;
    let darknet = state.store.get("darknet_count").await;
    let air_gapped = std::env::var("AEGIS_AIR_GAPPED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    Json(StatusResponse {
        oracle_alive: oracle,
        active_sentinels: sentinels,
        threats_blocked: blocked,
        osint_documents: osint,
        darknet_documents: darknet,
        shield_active: blocked > 0,
        version: "8.7.0".into(),
        air_gapped,
    })
}

async fn api_knowledge(
    State(_state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> Json<KnowledgeResponse> {
    Json(KnowledgeResponse {
        osint: vec!["CrowdStrike Falcon".into(), "SentinelOne Singularity".into(), "Palo Alto Cortex".into()],
        darknet: vec!["LockBit 4.0 TTPs".into(), "Golden SAML (T1606.002)".into(), "Supply Chain npm".into()],
    })
}

async fn api_threats(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> Json<Vec<ThreatResponse>> {
    let blocked = state.store.get("threats_blocked").await;
    let threats = if blocked > 0 {
        vec![
            ThreatResponse { id: "threat-001".into(), message: "LOTL: certutil.exe detected".into(), severity: 0.92, timestamp: "2026-05-08T12:00:00Z".into(), blocked: true },
        ]
    } else { vec![] };
    Json(threats)
}

async fn api_agents(
    State(state): State<AppState>,
    user: crate::auth::AuthUser,
) -> Json<Vec<AgentStatusResponse>> {
    // Minimal, deterministic telemetry for demo. No randomness to keep the demo reproducible.
    // Frontend expects: { id, role, status, load, critic }
    let oracle_alive = *state.oracle_alive.lock().unwrap();
    let active_sentinels = state.store.get("active_sentinels").await;
    let threats_blocked = state.store.get("threats_blocked").await;
    let osint = state.store.get("osint_count").await;
    let darknet = state.store.get("darknet_count").await;

    // Derive "tier" from JWT scope (issued by /api/login).
    let scope = &user.0.scope;
    let tier = if scope.iter().any(|s| s == "enterprise") {
        "enterprise"
    } else if scope.iter().any(|s| s == "pro") {
        "pro"
    } else {
        "starter"
    };

    let base_load = (active_sentinels % 100).max(10);
    let risk_hint: f64 = if threats_blocked > 0 { 0.86_f64 } else { 0.32_f64 };

    let agents = vec![
        AgentStatusResponse {
            id: "ORACLE-7".into(),
            role: format!("Threat Prediction ({})", tier),
            status: if oracle_alive { "OPERATIONAL".into() } else { "DEGRADED".into() },
            load: (base_load + (osint % 30)).min(99),
            critic: (0.90_f64 + (risk_hint * 0.05_f64)).min(0.99_f64),
        },
        AgentStatusResponse {
            id: "THREAT-HUNTER-3".into(),
            role: "IOC Correlation".into(),
            status: if osint + darknet > 0 { "HUNTING".into() } else { "IDLE".into() },
            load: (30 + (osint % 60)).min(99),
            critic: if threats_blocked > 0 { 0.91 } else { 0.86 },
        },
        AgentStatusResponse {
            id: "INQUISITOR-1".into(),
            role: "Deep Investigation".into(),
            status: if threats_blocked > 0 { "ANALYZING".into() } else { "STANDBY".into() },
            load: (40 + (threats_blocked % 50)).min(99),
            critic: if threats_blocked > 0 { 0.97 } else { 0.88 },
        },
        AgentStatusResponse {
            id: "SENTINEL-12".into(),
            role: "API Anomaly Detection".into(),
            status: "MONITORING".into(),
            load: (20 + ((active_sentinels + threats_blocked) % 40)).min(99),
            critic: 0.81,
        },
    ];

    Json(agents)
}

#[derive(Deserialize)]
struct AuditTailQuery {
    lines: Option<usize>,
}

async fn api_audit_tail(
    _state: State<AppState>,
    _user: crate::auth::AuthUser,
    axum::extract::Query(q): axum::extract::Query<AuditTailQuery>,
) -> impl IntoResponse {
    let n = q.lines.unwrap_or(12).min(200);
    let path = Path::new("./data/audit.log");
    if !path.exists() {
        return Json(serde_json::json!({ "lines": [], "path": "./data/audit.log", "exists": false }));
    }
    match fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().rev().take(n).collect();
            let out: Vec<String> = lines.into_iter().rev().map(|s| s.to_string()).collect();
            Json(serde_json::json!({ "lines": out, "path": "./data/audit.log", "exists": true }))
        }
        Err(e) => Json(serde_json::json!({ "lines": [], "path": "./data/audit.log", "exists": true, "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct CodeDemoRequest {
    task: String,
    approved: bool,
}

async fn api_code_demo(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<CodeDemoRequest>,
) -> impl IntoResponse {
    let action = format!("[HITL] /code demo requested: {}", req.task);
    if !req.approved {
        let _ = state.alert_tx.send(format!("{} -> HUMAN APPROVAL REQUIRED", action));
        return (
            axum::http::StatusCode::CONFLICT,
            Json(serde_json::json!({
                "status": "needs_human_approval",
                "message": "Human approval required for /code demo. Re-submit with approved=true."
            })),
        );
    }

    // Best-effort demo stream to WS; no external calls.
    let tx = state.alert_tx.clone();
    let task = req.task.clone();
    tokio::spawn(async move {
        let steps = vec![
            format!("[CODE] Approved by human. Task: {}", task),
            "[THOUGHT] Generating safe code snippet (offline, no external calls)...".into(),
            "[CRITIC] security_risk=0.12 | utility=0.86 -> ALLOW".into(),
            "[FINAL] Code generated and ready for review.".into(),
        ];
        for s in steps {
            let _ = tx.send(s);
            tokio::time::sleep(std::time::Duration::from_millis(650)).await;
        }
    });

    (
        axum::http::StatusCode::OK,
        Json(serde_json::json!({
            "status": "accepted",
            "message": "Code demo started. Watch War Room live feed."
        })),
    )
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut rx = state.alert_tx.subscribe();
    let blocked = state.store.get("threats_blocked").await;
    let init = serde_json::json!({
        "type": "init",
        "id": uuid::Uuid::new_v4().to_string(),
        "data": {
            "oracle_alive": *state.oracle_alive.lock().unwrap(),
            "active_sentinels": state.store.get("active_sentinels").await,
            "threats_blocked": blocked
        }
    });
    let init_id = init["id"].as_str().unwrap().to_string();
    {
        let mut p = state.pending_acks.lock().await;
        p.insert(init_id.clone(), (init.to_string(), std::time::Instant::now()));
    }
    let _ = socket.send(Message::Text(init.to_string())).await;

    // Фоновая очистка устаревших pending_acks (каждые 60 секунд)
    let pending_cleanup = state.pending_acks.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            ticker.tick().await;
            let mut p = pending_cleanup.lock().await;
            let now = std::time::Instant::now();
            p.retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < 600); // 10 минут TTL
        }
    });

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(alert) => {
                        let event_id = state.event_bus.publish("alert", &alert).await;
                        let msg = serde_json::json!({"type": "alert", "id": event_id, "data": alert});
                        {
                            let mut p = state.pending_acks.lock().await;
                            p.insert(event_id.clone(), (msg.to_string(), std::time::Instant::now()));
                        }
                        if socket.send(Message::Text(msg.to_string())).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(p) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(id) = p.get("ack").and_then(|v| v.as_str()) {
                                let mut pending = state.pending_acks.lock().await;
                                pending.remove(id);
                            }
                        }
                    }
                    _ => break,
                }
            }
        }
    }
}

async fn api_sync(State(state): State<AppState>, axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>) -> Json<Vec<serde_json::Value>> {
    let since = params.get("since").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let events = state.event_bus.get_since(since).await;
    Json(events.iter().map(|e| serde_json::json!({"id": e.id, "type": e.event_type, "payload": e.payload, "timestamp": e.timestamp})).collect())
}

async fn api_pilot_request(State(state): State<AppState>, Json(req): Json<PilotRequest>) -> impl IntoResponse {
    println!("[PILOT] {} ({}) — {}", req.name, req.email, req.company);
    let _ = state.alert_tx.send(format!("Новая заявка: {} из {}", req.name, req.company));
    Json(serde_json::json!({"success": true, "message": "Заявка принята."}))
}

async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buf = Vec::new();
    if encoder.encode(&metric_families, &mut buf).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "metrics encode error").into_response();
    }
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, encoder.format_type())],
        buf,
    )
        .into_response()
}

async fn metrics_handler_protected(_user: crate::auth::AuthUser) -> impl IntoResponse {
    metrics_handler().await
}

async fn security_headers_middleware(
    req: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();

    h.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    h.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );
    h
        .entry(HeaderName::from_static("cache-control"))
        .or_insert(HeaderValue::from_static("no-store"));

    res
}

#[derive(Deserialize)]
struct LogoutRequest {
    refresh_token: Option<String>,
}

async fn api_logout(
    State(state): State<AppState>,
    user: crate::auth::AuthUser,
    Json(req): Json<LogoutRequest>,
) -> impl IntoResponse {
    // Revoke access token jti
    if let Some(jti) = user.0.jti.as_deref() {
        if let Some(auth) = state.auth.as_deref() {
            crate::auth::revoke_token(auth, jti);
            if let Some(audit) = state.audit.as_deref() {
                let _ = audit.log_event("api", &format!("logout_revoke_access sub={}", user.0.sub), 0.2, true);
            }
        }
    }

    // Optionally revoke refresh token if provided
    if let (Some(refresh), Some(auth)) = (req.refresh_token, state.auth.as_deref()) {
        if let Ok(decoded) = jsonwebtoken::decode::<crate::auth::Claims>(
            &refresh,
            &auth.decoding_key,
            &auth.validation,
        ) {
            if decoded.claims.token_type == "refresh" {
                if let Some(jti) = decoded.claims.jti.as_deref() {
                    crate::auth::revoke_refresh_token(auth, jti, decoded.claims.exp as i64);
                    if let Some(audit) = state.audit.as_deref() {
                        let _ = audit.log_event("api", &format!("logout_revoke_refresh sub={}", user.0.sub), 0.3, true);
                    }
                }
            }
        }
    }

    Json(serde_json::json!({ "success": true }))
}

async fn api_fused_threats(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> Json<Vec<serde_json::Value>> {
    if let Some(fusion) = &state.fusion {
        let threats = fusion.get_fused_threats(20).await;
        let json = threats.into_iter().map(|t| serde_json::json!({
            "cluster_id": t.cluster_id,
            "severity": t.severity,
            "confidence": t.confidence,
            "sources": t.sources,
            "iocs": t.iocs,
            "summary": t.summary,
            "first_seen": t.first_seen,
            "last_seen": t.last_seen,
        })).collect();
        Json(json)
    } else {
        Json(vec![])
    }
}

// === Реакт endpoint для дашборда ===
#[derive(Deserialize)]
struct ReactRequest {
    mission: String,
}

async fn api_react(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<ReactRequest>,
) -> impl IntoResponse {
    let mission = req.mission.clone();
    let mission_for_task = mission.clone();
    let tx = state.alert_tx.clone();

    // Real ReAct++ execution simulation with live streaming to War Room
    tokio::spawn(async move {
        let steps = vec![
            format!("[ReAct++] Mission deployed: {}", mission_for_task),
            "[THOUGHT] Scanning 11 threat intel sources + internal knowledge base...".into(),
            "[CRITIC] security_risk=0.28 | utility=0.91 → VERDICT: ALLOW (MCTS score: 0.87)".into(),
            "[ACTION] Executing parallel ThreatHunter + FusionEngine correlation".into(),
            "[RESULT] 2 new high-severity clusters identified. Kill Switch not triggered.".into(),
            "[FINAL] Containment playbook generated and ready for operator approval.".into(),
        ];

        for step in steps {
            let _ = tx.send(step);
            tokio::time::sleep(std::time::Duration::from_millis(780)).await;
        }
    });

    Json(serde_json::json!({
        "status": "accepted",
        "mission": mission,
        "message": "ReAct++ agent deployed. Live reasoning steps are streaming to the War Room."
    }))
}

// === Scout / Ingest endpoints для дашборда ===
async fn api_scout(
    State(_state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    println!("[API] /api/scout triggered");
    Json(serde_json::json!({ "status": "scout_started" }))
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct IngestRequest {
    title: String,
    source: String,
    text: String,
}

async fn api_ingest(
    State(_state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<IngestRequest>,
) -> impl IntoResponse {
    // В реальности здесь будет вызов KnowledgeBase
    println!("[API] /api/ingest: {} from {}", req.title, req.source);
    Json(serde_json::json!({ "status": "ingested", "title": req.title }))
}

async fn api_ingest_darknet(
    State(_state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<IngestRequest>,
) -> impl IntoResponse {
    println!("[API] /api/ingest_darknet: {} from {}", req.title, req.source);
    Json(serde_json::json!({ "status": "ingested_darknet", "title": req.title }))
}

pub fn create_router(state: AppState) -> Router {
    // Rate limits (in-memory, per-IP + per-endpoint)
    const RL_LOGIN: RateLimitRule = RateLimitRule {
        name: "api_login",
        max_requests: 5,
        window: std::time::Duration::from_secs(5 * 60),
    };
    const RL_REFRESH: RateLimitRule = RateLimitRule {
        name: "api_refresh",
        max_requests: 30,
        window: std::time::Duration::from_secs(10 * 60),
    };
    const RL_PILOT: RateLimitRule = RateLimitRule {
        name: "api_pilot",
        max_requests: 10,
        window: std::time::Duration::from_secs(60 * 60),
    };
    const RL_REACT: RateLimitRule = RateLimitRule {
        name: "api_react",
        max_requests: 30,
        window: std::time::Duration::from_secs(10 * 60),
    };
    const RL_STATUS: RateLimitRule = RateLimitRule {
        name: "api_status",
        max_requests: 60,
        window: std::time::Duration::from_secs(60),
    };
    const RL_METRICS: RateLimitRule = RateLimitRule {
        name: "metrics",
        max_requests: 60,
        window: std::time::Duration::from_secs(60),
    };

    let is_dev = std::env::var("AEGIS_DEV_MODE").map(|v| v == "1").unwrap_or(false);

    // === ЗАЩИЩЁННЫЕ FEDERATION РОУТЫ ===
    let federation = Router::new()
        .route("/federation/merkle", get(federation_merkle_handler))
        .route("/federation/items", get(federation_items_handler))
        .route("/federation/changed_since", axum::routing::post(federation_changed_since_handler))
        .route("/federation/missing", axum::routing::post(federation_missing_handler))
        .layer(axum::middleware::from_fn(crate::mtls::mtls_auth_layer));

    let public_routes = Router::new()
        .route("/health", get(health_check))
        .route(
            "/api/pilot",
            axum::routing::post(api_pilot_request).route_layer(from_fn_with_state(
                RateLimitState {
                    limiter: state.rate_limiter.clone(),
                    rule: RL_PILOT,
                },
                rate_limit_middleware,
            )),
        )
        .route(
            "/api/login",
            axum::routing::post(login).route_layer(from_fn_with_state(
                RateLimitState {
                    limiter: state.rate_limiter.clone(),
                    rule: RL_LOGIN,
                },
                rate_limit_middleware,
            )),
        )
        .route(
            "/api/refresh",
            axum::routing::post(crate::auth::refresh).route_layer(from_fn_with_state(
                RateLimitState {
                    limiter: state.rate_limiter.clone(),
                    rule: RL_REFRESH,
                },
                rate_limit_middleware,
            )),
        )
        ;

    let protected_routes = Router::new()
        // Metrics: public only in dev; in prod require JWT
        .route(
            "/metrics",
            if is_dev { get(metrics_handler) } else { get(metrics_handler_protected) },
        )
        .route_layer(from_fn_with_state(
            RateLimitState {
                limiter: state.rate_limiter.clone(),
                rule: RL_METRICS,
            },
            rate_limit_middleware,
        ))
        .route(
            "/api/status",
            get(api_status).route_layer(from_fn_with_state(
                RateLimitState {
                    limiter: state.rate_limiter.clone(),
                    rule: RL_STATUS,
                },
                rate_limit_middleware,
            )),
        )
        .route("/api/sync", get(api_sync))
        .route("/api/knowledge", get(api_knowledge))
        .route("/api/threats", get(api_threats))
        .route("/api/agents", get(api_agents))
        .route("/api/audit-tail", get(api_audit_tail))
        .route("/api/fused-threats", get(api_fused_threats))
        .route("/api/logout", axum::routing::post(api_logout))
        .route(
            "/api/react",
            axum::routing::post(api_react).route_layer(from_fn_with_state(
                RateLimitState {
                    limiter: state.rate_limiter.clone(),
                    rule: RL_REACT,
                },
                rate_limit_middleware,
            )),
        )
        .route("/api/code-demo", axum::routing::post(api_code_demo))
        .route("/api/scout", axum::routing::post(api_scout))
        .route("/api/ingest", axum::routing::post(api_ingest))
        .route("/api/ingest_darknet", axum::routing::post(api_ingest_darknet))
        .route("/ws", get(ws_handler));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .nest("/", federation)
        .with_state(state.clone());

    // Attach AuthState to extensions so AuthUser extractor works
    let app = if let Some(auth) = state.auth.clone() {
        app.layer(axum::Extension(auth))
    } else {
        app
    };

    // Attach AuditTrail to extensions (optional)
    let app = if let Some(audit) = state.audit.clone() {
        app.layer(axum::Extension(audit))
    } else {
        app
    };

    // CORS + minimal security headers
    let app = app.layer(axum::middleware::from_fn(security_headers_middleware));

    if is_dev {
        app.layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
    } else {
        let origin = std::env::var("AEGIS_ALLOWED_ORIGIN")
            .unwrap_or_else(|_| "https://aegis-security.ru".to_string());
        let allowed_origin = origin
            .parse::<HeaderValue>()
            .unwrap_or_else(|_| HeaderValue::from_static("https://aegis-security.ru"));
        app.layer(
            CorsLayer::new()
                .allow_origin(allowed_origin)
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                .allow_headers([axum::http::header::AUTHORIZATION, axum::http::header::CONTENT_TYPE]),
        )
    }
}

pub async fn start_server(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let app = create_router(state);
    let port: u16 = std::env::var("AEGIS_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    println!("🌐 AEGIS API Server on http://0.0.0.0:{}", port);
    println!("   /api/login — получить JWT (access + refresh)");
    println!("   /api/refresh — обновить access token");
    println!("   /api/status, /api/knowledge, /api/threats, /api/fused-threats, /api/react, /api/scout, /api/ingest* — ЗАЩИЩЕНЫ JWT");
    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            // If port is already in use, keep CLI running; HTTP API is optional for Phase 1.
            if e.kind() == std::io::ErrorKind::AddrInUse {
                tracing::warn!("HTTP server disabled ({}): {}", bind_addr, e);
                return Ok(());
            }
            return Err(Box::new(e));
        }
    };
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}

async fn federation_merkle_handler(
    State(state): State<AppState>,
) -> axum::response::Response {
    let federation = match state.federation.as_ref() {
        Some(f) => f,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "FederationLayer is not initialized in AppState" }))
            ).into_response();
        }
    };
    
    match federation.get_merkle_root().await {
        Ok(root) => Json(serde_json::json!({ "root": root })).into_response(),
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to get merkle root: {}", e) }))
            ).into_response()
        }
    }
}

async fn federation_items_handler(
    State(state): State<AppState>,
) -> axum::response::Response {
    let federation = match state.federation.as_ref() {
        Some(f) => f,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "FederationLayer is not initialized in AppState" }))
            ).into_response();
        }
    };
    
    // Временно делаем метод get_all_white публичным, обращаясь к kb (потребует pub(crate) kb в FederationLayer, либо добавим методы-обертки позже)
    // Но так как у нас сейчас kb публичным не объявлялся, сделаем его pub
    let white = match federation.kb.get_all_white().await {
        Ok(w) => w,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to get white items: {}", e) }))
            ).into_response();
        }
    };

    let black = match federation.kb.get_all_black().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to get black items: {}", e) }))
            ).into_response();
        }
    };

    Json(serde_json::json!({
        "white": white,
        "black": black
    })).into_response()
}

async fn federation_changed_since_handler(
    State(state): State<AppState>,
    Json(since): Json<i64>,
) -> axum::response::Response {
    let federation = match state.federation.as_ref() {
        Some(f) => f,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "FederationLayer is not initialized in AppState" }))).into_response(),
    };
    
    match federation.kb.get_changed_since(since).await {
        Ok(items) => Json(items).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Failed to get changed items: {}", e) }))).into_response(),
    }
}

async fn federation_missing_handler(
    State(state): State<AppState>,
    Json(their_hashes): Json<Vec<String>>,
) -> axum::response::Response {
    let federation = match state.federation.as_ref() {
        Some(f) => f,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "FederationLayer is not initialized in AppState" }))).into_response(),
    };
    
    let my_hashes = match federation.kb.get_all_hashes().await {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Failed to get hashes: {}", e) }))).into_response(),
    };
    let my_set: std::collections::HashSet<String> = my_hashes.into_iter().map(|(_, h)| h).collect();

    let mut missing = Vec::new();
    
    for hash in their_hashes {
        if !my_set.contains(&hash) {
            if let Ok(Some(item)) = federation.kb.get_by_content_hash(&hash).await {
                missing.push(item);
            }
        }
    }
    
    Json(missing).into_response()
}