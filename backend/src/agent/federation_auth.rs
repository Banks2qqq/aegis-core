//! PR5 — Federation peer authentication (shared secret + per-peer tokens).

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::config::{FederationConfig, RunMode};

pub const FEDERATION_TOKEN_HEADER: &str = "x-aegis-federation-token";

#[derive(Clone)]
pub struct FederationAuthState {
    accepted_tokens: Vec<String>,
    /// Dev only: allow federation routes without token when no tokens configured.
    allow_insecure: bool,
    /// Production without tokens → 503 (fail-closed).
    production_strict: bool,
}

impl FederationAuthState {
    pub fn from_federation(cfg: &FederationConfig, mode: &RunMode) -> Self {
        let allow_insecure = std::env::var("AEGIS_FEDERATION_INSECURE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let production_strict = *mode == RunMode::Production;
        let accepted_tokens = cfg.accepted_tokens();
        if production_strict && accepted_tokens.is_empty() && !allow_insecure {
            tracing::error!(
                "Federation: production mode requires FEDERATION_SHARED_SECRET (or AEGIS_FEDERATION_INSECURE=1 for dev)"
            );
        }
        Self {
            accepted_tokens,
            allow_insecure,
            production_strict,
        }
    }

    /// Kept for tests / legacy call sites.
    pub fn from_secret(secret: Option<String>) -> Self {
        let mut cfg = FederationConfig::default();
        cfg.shared_secret = secret;
        Self::from_federation(&cfg, &RunMode::Development)
    }

    pub fn routes_enabled(&self) -> bool {
        if self.production_strict && self.accepted_tokens.is_empty() && !self.allow_insecure {
            return false;
        }
        true
    }

    pub fn auth_configured(&self) -> bool {
        !self.accepted_tokens.is_empty()
    }

    pub fn validate_token(&self, presented: Option<&str>) -> bool {
        if !self.routes_enabled() {
            return false;
        }
        match presented {
            Some(got) => self
                .accepted_tokens
                .iter()
                .any(|expected| constant_time_eq(got, expected)),
            None => self.allow_insecure && self.accepted_tokens.is_empty(),
        }
    }
}

pub async fn federation_peer_auth_middleware(
    State(auth): State<FederationAuthState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !auth.routes_enabled() {
        tracing::warn!("Federation routes disabled (production, no shared secret)");
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let token = request
        .headers()
        .get(FEDERATION_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok());

    if auth.validate_token(token) {
        log_federation_mtls_client(&request);
        Ok(next.run(request).await)
    } else {
        tracing::warn!("Federation peer auth rejected");
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Log nginx-forwarded client cert metadata (port 8443 mTLS listener).
fn log_federation_mtls_client(request: &Request) {
    let verify = header_str(request, "x-ssl-client-verify");
    let subject = header_str(request, "x-ssl-client-s-dn");
    let fingerprint = header_str(request, "x-ssl-client-fingerprint");
    let peer = header_str(request, "x-forwarded-for")
        .or_else(|| header_str(request, "x-real-ip"));

    if verify.as_deref() == Some("SUCCESS") {
        crate::metrics::federation_mtls_inbound("success");
        tracing::info!(
            peer_ip = ?peer,
            ssl_subject = ?subject,
            ssl_fingerprint = ?fingerprint,
            path = %request.uri().path(),
            "Federation mTLS peer connected"
        );
    } else if verify.is_some() {
        crate::metrics::federation_mtls_inbound("rejected");
        tracing::warn!(
            peer_ip = ?peer,
            ssl_verify = ?verify,
            ssl_subject = ?subject,
            ssl_fingerprint = ?fingerprint,
            path = %request.uri().path(),
            "Federation request without valid client certificate"
        );
    } else {
        crate::metrics::federation_mtls_inbound("none");
    }
}

fn header_str(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
