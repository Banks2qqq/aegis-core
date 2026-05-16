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

/// Последняя завершённая ReAct++ миссия (для /api/agents и дашборда).
#[derive(Clone, Serialize)]
pub struct ReactMissionSnapshot {
    pub mission: String,
    pub success: bool,
    pub iterations_used: u32,
    pub completed_at: i64,
    pub final_answer: String,
}
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::auth::login;
use crate::persistence::PersistentStore;
use crate::event_bus::EventBus;
use crate::fusion_engine::FusionEngine;
use crate::fstec_bdu::BduVulnerability;
use crate::honeypot_manager::{HoneypotManager, HoneypotType};
use crate::config::AEGISConfig;
use crate::distributed_oracle::ConsensusLayer;
use crate::distributed_oracle::DistributedOracle;
use crate::healing_orchestrator::HealingOrchestrator;
use crate::isolation::{AdaptiveIsolation, NetworkPolicy, Workload};
use crate::knowledge::KnowledgeBase;
use crate::agent_registry::{AgentDashboardContext, AgentRegistry, ReactRunMeta, ScoutRunMeta};
use crate::react_service::ReactService;
use crate::learning_orchestrator::LearningOrchestrator;
use crate::scout_pipeline;
use crate::scout_report::{self, ScoutOperatorReport};
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

    // Local monitoring / alert scripts on same host — no login rate cap
    if ip == "127.0.0.1" || ip == "::1" {
        return next.run(req).await;
    }

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
    pub kb: Option<Arc<KnowledgeBase>>,
    pub learning: Option<Arc<LearningOrchestrator>>,
    pub honeypots: Option<Arc<HoneypotManager>>,
    pub air_gapped: Arc<Mutex<bool>>,
    /// PR3.1 — operational agent registry (Scout, ThreatHunter, Healer, MTD)
    pub agent_registry: Arc<AgentRegistry>,
    /// Последний результат Scout v0.5 (ФСТЭК БДУ) для GET /api/bdu/recent
    pub bdu_cache: Arc<Mutex<BduCacheState>>,
    pub oracle: Option<Arc<DistributedOracle>>,
    pub healing: Option<Arc<HealingOrchestrator>>,
    pub config: Option<Arc<AEGISConfig>>,
    /// Shared with P2P discovery — source of truth for Raft status.
    pub raft: Option<Arc<Mutex<ConsensusLayer>>>,
    pub react: Option<Arc<ReactService>>,
    pub last_react: Arc<Mutex<Option<ReactMissionSnapshot>>>,
    rate_limiter: Arc<RateLimiter>,
}

#[derive(Clone, Default)]
pub struct BduCacheState {
    pub items: Vec<BduVulnerability>,
    pub last_scout: Option<BduLastScoutMeta>,
}

#[derive(Clone, Serialize)]
pub struct BduLastScoutMeta {
    pub completed_at: i64,
    pub found: usize,
    pub ingested: usize,
    pub ingested_new: usize,
    pub ingested_updated: usize,
    pub fusion_updated: usize,
    pub deception_deployed: usize,
    pub healing_attempted: usize,
    pub healing_applied: usize,
    pub status: String,
    #[serde(default)]
    pub total_findings: usize,
    #[serde(default)]
    pub sources_ok: usize,
    #[serde(default)]
    pub sources_skipped: usize,
    #[serde(default)]
    pub sources_failed: usize,
    #[serde(default)]
    pub critic_verdict: String,
    #[serde(default)]
    pub critic_risk: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<ScoutOperatorReport>,
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
            kb: None,
            learning: None,
            honeypots: None,
            air_gapped: Arc::new(Mutex::new(false)),
            agent_registry: Arc::new(AgentRegistry::new()),
            bdu_cache: Arc::new(Mutex::new(BduCacheState::default())),
            oracle: None,
            healing: None,
            config: None,
            raft: None,
            react: None,
            last_react: Arc::new(Mutex::new(None)),
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

    pub fn with_kb(mut self, kb: Arc<KnowledgeBase>) -> Self {
        self.kb = Some(kb);
        self
    }

    pub fn with_learning(mut self, learning: Arc<LearningOrchestrator>) -> Self {
        self.learning = Some(learning);
        self
    }

    pub fn with_honeypots(mut self, honeypots: Arc<HoneypotManager>) -> Self {
        self.honeypots = Some(honeypots);
        self
    }

    pub fn with_oracle(mut self, oracle: Arc<DistributedOracle>) -> Self {
        self.oracle = Some(oracle);
        self
    }

    pub fn with_healing(mut self, healing: Arc<HealingOrchestrator>) -> Self {
        self.healing = Some(healing);
        self
    }

    pub fn with_config(mut self, config: Arc<AEGISConfig>) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_raft(mut self, raft: Arc<Mutex<ConsensusLayer>>) -> Self {
        self.raft = Some(raft);
        self
    }

    pub fn with_react(mut self, react: Arc<ReactService>) -> Self {
        self.react = Some(react);
        self
    }

    pub fn with_agent_registry(mut self, registry: Arc<AgentRegistry>) -> Self {
        self.agent_registry = registry;
        self
    }
}

async fn build_agent_dashboard_ctx(state: &AppState) -> AgentDashboardContext {
    let black_kb = if let Some(kb) = &state.kb {
        kb.count_black().await.unwrap_or(0)
    } else {
        0
    };
    let fusion_clusters = if let Some(fusion) = &state.fusion {
        fusion
            .get_stats()
            .await
            .get("active_clusters")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize
    } else {
        0
    };
    let honeypots = if let Some(hp) = &state.honeypots {
        hp.list_active().await.len()
    } else {
        0
    };
    let last_scout = state.bdu_cache.lock().await.last_scout.as_ref().map(|m| ScoutRunMeta {
        completed_at: m.completed_at,
        found: m.found,
        fusion_updated: m.fusion_updated,
        healing_attempted: m.healing_attempted,
        healing_applied: m.healing_applied,
        status: m.status.clone(),
    });
    let last_react = state.last_react.lock().await.as_ref().map(|r| ReactRunMeta {
        mission: r.mission.clone(),
        success: r.success,
        iterations_used: r.iterations_used,
        completed_at: r.completed_at,
    });
    AgentDashboardContext {
        black_kb,
        fusion_clusters,
        honeypots,
        healing_ready: state.healing.is_some(),
        react_ready: state.react.is_some(),
        last_scout,
        last_react,
    }
}

const AEGIS_API_VERSION: &str = "8.7.0";

fn network_policy_label(policy: &NetworkPolicy) -> &'static str {
    match policy {
        NetworkPolicy::Full => "full",
        NetworkPolicy::OutboundOnly => "outbound_only",
        NetworkPolicy::Isolated => "isolated",
        NetworkPolicy::None => "none",
    }
}

#[derive(Serialize)]
struct StatusResponse {
    oracle_alive: bool,
    active_sentinels: u64,
    threats_blocked: u64,
    osint_documents: u64,
    darknet_documents: u64,
    black_kb_count: u64,
    bdu_kb_count: u64,
    fusion_clusters: u64,
    shield_active: bool,
    version: String,
    air_gapped: bool,
    react_ready: bool,
    llm_ready: bool,
    llm_cloud_available: bool,
    llm_local_available: bool,
}

#[derive(Serialize)]
struct KnowledgeResponse {
    bdu: Vec<String>,
    other_intel: Vec<String>,
    black_kb_count: usize,
    /// Legacy keys (same slices as `bdu` / `other_intel`).
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

#[allow(dead_code)]
#[derive(Serialize)]
struct AgentStatusResponse {
    id: String,
    role: String,
    status: String,
    load: u64,
    critic: f64,
}

#[derive(Deserialize, Serialize)]
struct PilotRequest {
    name: String,
    company: String,
    email: String,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "oracle": "alive",
        "version": AEGIS_API_VERSION
    }))
}

async fn api_status(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,   // JWT required
) -> Json<StatusResponse> {
    let oracle = *state.oracle_alive.lock().unwrap();
    let blocked = state.store.get("threats_blocked").await;

    let mut sentinels: u64 = 0;
    if let Some(hp) = &state.honeypots {
        sentinels = hp.list_active().await.len() as u64;
    }
    let ctx = build_agent_dashboard_ctx(&state).await;
    let agent_list = state.agent_registry.dashboard_agents(ctx).await;
    sentinels += agent_list
        .iter()
        .filter(|a| a.get("status").and_then(|s| s.as_str()) == Some("active"))
        .count() as u64;

    let mut osint: u64 = state.store.get("osint_count").await;
    let mut darknet: u64 = state.store.get("darknet_count").await;
    let mut black_kb_count: u64 = 0;
    let mut bdu_kb_count: u64 = 0;
    let mut fusion_clusters: u64 = 0;
    let mut shield_active = blocked > 0;
    if let Some(kb) = &state.kb {
        if osint == 0 {
            osint = kb.count_legacy_documents("osint").await.unwrap_or(0) as u64;
        }
        if darknet == 0 {
            darknet = kb.count_legacy_documents("darknet").await.unwrap_or(0) as u64;
        }
        black_kb_count = kb.count_black().await.unwrap_or(0) as u64;
        bdu_kb_count = kb.count_black_by_source("fstec_bdu").await.unwrap_or(0) as u64;
        if bdu_kb_count > 0 && osint == 0 {
            osint = bdu_kb_count;
        }
        if black_kb_count > bdu_kb_count && darknet == 0 {
            darknet = black_kb_count.saturating_sub(bdu_kb_count);
        }
        if black_kb_count > 0 {
            shield_active = true;
        }
    }
    if let Some(fusion) = &state.fusion {
        let stats = fusion.get_stats().await;
        fusion_clusters = stats
            .get("active_clusters")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if fusion_clusters > 0 {
            shield_active = true;
        }
    }

    let air_gapped = *state.air_gapped.lock().await;
    let llm = crate::llm_status::probe_llm(
        state.react.as_ref(),
        state.config.as_ref(),
        None,
        None,
    )
    .await;
    Json(StatusResponse {
        oracle_alive: oracle,
        active_sentinels: sentinels,
        threats_blocked: blocked,
        osint_documents: osint,
        darknet_documents: darknet,
        black_kb_count,
        bdu_kb_count,
        fusion_clusters,
        shield_active,
        version: AEGIS_API_VERSION.into(),
        air_gapped,
        react_ready: llm.react_ready,
        llm_ready: llm.llm_ready,
        llm_cloud_available: llm.cloud_available,
        llm_local_available: llm.local_available,
    })
}

async fn api_react_status(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    if let Some(react) = &state.react {
        return Json(react.readiness().await).into_response();
    }
    Json(
        crate::llm_status::probe_llm(None, state.config.as_ref(), None, None).await,
    )
    .into_response()
}

async fn api_knowledge(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> Json<KnowledgeResponse> {
    let Some(kb) = &state.kb else {
        return Json(KnowledgeResponse {
            bdu: vec![],
            other_intel: vec![],
            black_kb_count: 0,
            osint: vec![],
            darknet: vec![],
        });
    };
    let black = kb.get_all_black().await.unwrap_or_default();
    let fmt_item = |i: &crate::knowledge_item::KnowledgeItem, max: usize| {
        format!(
            "{} — {}",
            i.tags
                .first()
                .map(|t| t.as_str())
                .unwrap_or(i.source.as_str()),
            i.summary
                .as_deref()
                .unwrap_or(&i.content)
                .chars()
                .take(max)
                .collect::<String>()
        )
    };
    let bdu: Vec<String> = black
        .iter()
        .filter(|i| i.source == "fstec_bdu")
        .take(15)
        .map(|i| fmt_item(i, 120))
        .collect();
    let other_intel: Vec<String> = black
        .iter()
        .filter(|i| i.source != "fstec_bdu")
        .take(10)
        .map(|i| fmt_item(i, 100))
        .collect();
    Json(KnowledgeResponse {
        osint: bdu.clone(),
        darknet: other_intel.clone(),
        black_kb_count: black.len(),
        bdu,
        other_intel,
    })
}

async fn api_threats(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> Json<Vec<ThreatResponse>> {
    let mut threats = Vec::new();
    if let Some(fusion) = &state.fusion {
        for t in fusion.get_fused_threats(25).await {
            let ts = chrono::DateTime::from_timestamp(t.last_seen, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| t.last_seen.to_string());
            let contained = fusion.is_contained(&t.cluster_id).await;
            threats.push(ThreatResponse {
                id: t.cluster_id.clone(),
                message: t.summary.clone(),
                severity: t.severity,
                timestamp: ts,
                blocked: contained,
            });
        }
    }
    Json(threats)
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
    let data_root = agent_data_root(&state);
    let id = uuid::Uuid::new_v4().to_string();
    let dir = data_root.join("pilot_requests");
    if let Err(e) = fs::create_dir_all(&dir) {
        tracing::error!("pilot_request mkdir: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "message": "storage error"})),
        )
            .into_response();
    }
    let record = serde_json::json!({
        "id": id,
        "received_at": chrono::Utc::now().timestamp(),
        "name": req.name,
        "company": req.company,
        "email": req.email,
        "phone": req.phone,
        "message": req.message,
    });
    let path = dir.join(format!("{}.json", id));
    if let Err(e) = fs::write(&path, serde_json::to_string_pretty(&record).unwrap_or_default()) {
        tracing::error!("pilot_request write: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "message": "storage error"})),
        )
            .into_response();
    }
    if let Some(audit) = &state.audit {
        let _ = audit.log_event(
            "pilot_request",
            &format!(
                "id={} name={} company={} email={}",
                id, req.name, req.company, req.email
            ),
            0.15,
            false,
        );
    }
    let _ = state
        .alert_tx
        .send(format!("Новая заявка на пилот: {} ({})", req.name, req.company));
    Json(serde_json::json!({
        "success": true,
        "message": "Заявка принята.",
        "request_id": id,
    }))
    .into_response()
}

/// Public metrics for landing (no auth, no marketing inflation).
async fn api_status_public(State(state): State<AppState>) -> impl IntoResponse {
    let mut bdu_kb_count: u64 = 0;
    let mut fusion_clusters: u64 = 0;
    let mut federation_peers: u64 = 0;
    let mut honeypots_active: u64 = 0;

    if let Some(kb) = &state.kb {
        bdu_kb_count = kb.count_black_by_source("fstec_bdu").await.unwrap_or(0) as u64;
    }
    if let Some(fusion) = &state.fusion {
        let stats = fusion.get_stats().await;
        fusion_clusters = stats
            .get("active_clusters")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
    }
    if let Some(fed) = &state.federation {
        federation_peers = fed.configured_peers().len() as u64;
    }
    if let Some(hp) = &state.honeypots {
        honeypots_active = hp.list_active().await.len() as u64;
    }

    Json(serde_json::json!({
        "status": "ok",
        "version": AEGIS_API_VERSION,
        "bdu_records": bdu_kb_count,
        "fusion_clusters": fusion_clusters,
        "federation_peers": federation_peers,
        "honeypots_active": honeypots_active,
        "healing_ready": state.healing.is_some(),
    }))
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
    let mut json = Vec::new();

    if let Some(fusion) = &state.fusion {
        let threats = fusion.get_fused_threats(20).await;
        for t in threats {
            let iocs: Vec<String> = t
                .iocs
                .iter()
                .map(|ioc| format!("{}:{}", ioc.ioc_type, ioc.value))
                .collect();
            let first_seen = chrono::DateTime::from_timestamp(t.first_seen, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| t.first_seen.to_string());
            let last_seen = chrono::DateTime::from_timestamp(t.last_seen, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| t.last_seen.to_string());
            let contained = fusion.is_contained(&t.cluster_id).await;
            json.push(serde_json::json!({
                "cluster_id": t.cluster_id,
                "severity": t.severity,
                "confidence": t.confidence,
                "sources": t.sources,
                "iocs": iocs,
                "summary": t.summary,
                "first_seen": first_seen,
                "last_seen": last_seen,
                "contained": contained,
            }));
        }
    }
    Json(json)
}

async fn api_federation_nodes(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> Json<Vec<serde_json::Value>> {
    if let Some(fed) = &state.federation {
        let nodes = fed.get_all_nodes().await;
        Json(nodes)
    } else {
        Json(vec![])
    }
}

async fn api_raft_status(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    let Some(raft) = &state.raft else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "raft_not_initialized",
                "leader_id": null,
                "term": 0,
                "commit_index": 0,
                "last_applied": 0,
                "log_size": 0,
                "last_log_index": 0,
                "active_nodes": 0,
                "total_nodes": 0,
                "nodes": []
            })),
        )
            .into_response();
    };
    let snap = raft.lock().await.status_snapshot();
    Json(snap).into_response()
}

async fn api_raft_metrics(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    let Some(raft) = &state.raft else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "raft_not_initialized" })),
        )
            .into_response();
    };
    let metrics = raft.lock().await.metrics_snapshot();
    Json(metrics).into_response()
}

#[derive(Deserialize)]
struct SyncRequest {
    peer_url: Option<String>,
    peer_id: Option<String>,
    #[serde(default)]
    sync_all: bool,
}

async fn api_federation_merkle(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    let Some(fed) = &state.federation else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "federation_not_initialized" })),
        )
            .into_response();
    };
    match fed.get_merkle_root().await {
        Ok(merkle_root) => Json(serde_json::json!({
            "local_node_id": fed.local_node_id(),
            "merkle_root": merkle_root,
            "auth_enabled": fed.auth_enabled(),
            "mtls_enabled": fed.mtls_enabled(),
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn api_federation_metrics(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    let Some(fed) = &state.federation else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "federation_not_initialized" })),
        )
            .into_response();
    };
    Json(fed.ops_metrics().await).into_response()
}

async fn api_federation_health(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    let Some(fed) = &state.federation else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "federation_not_initialized"
            })),
        )
            .into_response();
    };
    let report = fed.health_report().await;
    let raft = if let Some(raft) = &state.raft {
        Some(serde_json::to_value(raft.lock().await.status_snapshot()).unwrap_or_default())
    } else {
        None
    };
    Json(serde_json::json!({
        "report": report,
        "raft": raft,
    }))
    .into_response()
}

async fn api_federation_sync(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<SyncRequest>,
) -> impl IntoResponse {
    let Some(fed) = &state.federation else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "success": false,
                "error": "federation_not_initialized"
            })),
        )
            .into_response();
    };

    if req.sync_all {
        let results = fed.sync_all_peers().await;
        let ok = results.iter().filter(|r| r.success).count();
        let _ = state.alert_tx.send(format!(
            "[FEDERATION] sync_all complete: {}/{} peers",
            ok,
            results.len()
        ));
        return Json(serde_json::json!({
            "success": ok > 0 || results.is_empty(),
            "sync_all": true,
            "results": results,
        }))
        .into_response();
    }

    let outcome = if let Some(id) = req.peer_id.filter(|s| !s.trim().is_empty()) {
        fed.sync_with_peer_id(id.trim()).await
    } else if let Some(url) = req.peer_url.filter(|s| !s.trim().is_empty()) {
        fed.sync_with_peer(url.trim()).await
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "peer_url, peer_id, or sync_all required"
            })),
        )
            .into_response();
    };

    match outcome {
        Ok(r) => {
            let _ = state.alert_tx.send(format!(
                "[FEDERATION] sync {} → {} items (raft={:?})",
                r.peer_id, r.synced, r.raft_index
            ));
            Json(serde_json::json!({
                "success": r.success,
                "result": r,
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "success": false,
                "error": e
            })),
        )
            .into_response(),
    }
}

async fn api_agents(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> Json<Vec<serde_json::Value>> {
    let ctx = build_agent_dashboard_ctx(&state).await;
    Json(state.agent_registry.dashboard_agents(ctx).await)
}

async fn api_agents_action(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    axum::extract::Path((id, action)): axum::extract::Path<(String, String)>,
) -> impl IntoResponse {
    let enabled = action == "start";
    if !state.agent_registry.set_enabled(&id, enabled).await {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "success": false, "error": "unknown agent" })),
        );
    }
    let new_status = if enabled { "active" } else { "paused" };
    let _ = state.alert_tx.send(format!(
        "[AGENT] {} → {} (action: {})",
        id, new_status, action
    ));
    println!("[API] agent {} → {}", id, new_status);
    (
        axum::http::StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "id": id,
            "action": action,
            "status": new_status
        })),
    )
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
    let mission = req.mission.trim().to_string();
    if mission.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "error",
                "message": "mission must not be empty"
            })),
        )
            .into_response();
    }

    let Some(react) = state.react.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "message": "ReAct runtime not initialized"
            })),
        )
            .into_response();
    };

    let tx = state.alert_tx.clone();
    let audit = state.audit.clone();
    let last_react = state.last_react.clone();
    let mission_for_task = mission.clone();
    tokio::spawn(async move {
        let result = react.run_mission(&mission_for_task, tx).await;
        {
            let mut slot = last_react.lock().await;
            *slot = Some(ReactMissionSnapshot {
                mission: mission_for_task.clone(),
                success: result.success,
                iterations_used: result.iterations_used,
                completed_at: chrono::Utc::now().timestamp(),
                final_answer: result.final_answer.clone(),
            });
        }
        if let Some(audit) = audit {
            let _ = audit.log_event(
                "react_mission",
                &format!(
                    "mission={} success={} iterations={}",
                    mission_for_task, result.success, result.iterations_used
                ),
                if result.success { 0.2 } else { 0.75 },
                result.success,
            );
        }
    });

    Json(serde_json::json!({
        "status": "accepted",
        "mission": mission,
        "message": "ReAct++ mission started — live steps stream to War Room."
    }))
    .into_response()
}

// === Scout v0.5 — ФСТЭК БДУ (bdu.fstec.ru) ===
#[derive(Serialize)]
struct ScoutResponse {
    status: &'static str,
    found: usize,
    ingested: usize,
    ingested_new: usize,
    ingested_updated: usize,
    source: &'static str,
    completed_at: i64,
    items: Vec<BduVulnerability>,
    critic_verdict: String,
    critic_risk: f64,
    inquisitor_blocks: usize,
    inquisitor_escalates: usize,
    fusion_updated: usize,
    deception_deployed: usize,
    healing_attempted: usize,
    healing_applied: usize,
    /// Scout 2.0 multi-source stats
    #[serde(default)]
    total_findings: usize,
    #[serde(default)]
    sources_ok: usize,
    #[serde(default)]
    sources_skipped: usize,
    #[serde(default)]
    sources_failed: usize,
    #[serde(default)]
    sources: Vec<serde_json::Value>,
    #[serde(default)]
    enrichment_merged: usize,
    #[serde(default)]
    total_iocs: usize,
    #[serde(default)]
    total_cves: usize,
    pipeline: Vec<&'static str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    report: Option<ScoutOperatorReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct BduRecentResponse {
    items: Vec<BduVulnerability>,
    last_scout: Option<BduLastScoutMeta>,
}

async fn api_bdu_recent(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> Json<BduRecentResponse> {
    let cache = state.bdu_cache.lock().await;
    let mut items = cache.items.clone();
    items.truncate(20);
    Json(BduRecentResponse {
        items,
        last_scout: cache.last_scout.clone(),
    })
}

async fn store_bdu_cache(state: &AppState, outcome: &scout_pipeline::PipelineOutcome, completed_at: i64) {
    let mut cache = state.bdu_cache.lock().await;
    cache.items = outcome.vulns.clone();
    cache.items.truncate(20);
    let ingested = outcome.cycle.ingested_ok;
    let report = scout_report::build_operator_report(outcome, &outcome.scheduled_threats);
    cache.last_scout = Some(BduLastScoutMeta {
        completed_at,
        found: outcome.findings.len(),
        ingested,
        ingested_new: outcome.cycle.ingested_new,
        ingested_updated: outcome.cycle.ingested_updated,
        fusion_updated: outcome.fusion_updated,
        deception_deployed: outcome.deception_deployed,
        healing_attempted: outcome.healing_attempted,
        healing_applied: outcome.healing_applied,
        status: "success".into(),
        total_findings: outcome.findings.len(),
        sources_ok: outcome.sources_ok,
        sources_skipped: outcome.sources_skipped,
        sources_failed: outcome.sources_failed,
        critic_verdict: outcome.cycle.critic_verdict.clone(),
        critic_risk: outcome.cycle.critic_risk,
        report: Some(report),
    });
}

async fn api_scout(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    let tx = state.alert_tx.clone();
    let completed_at = chrono::Utc::now().timestamp();

    let Some(learning) = state.learning.clone() else {
        let err = "LearningOrchestrator not initialized".to_string();
        let _ = tx.send(format!("[SCOUT] ✗ {}", err));
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(empty_scout_error(&err, completed_at)),
        );
    };

    match scout_pipeline::run_fstec_immunity_pipeline(
        &learning,
        state.fusion.clone(),
        state.honeypots.clone(),
        state.healing.clone(),
        Some(state.agent_registry.clone()),
        tx.clone(),
    )
    .await
    {
        Ok(outcome) => {
            let ingested = outcome.cycle.ingested_ok;
            let ingested_new = outcome.cycle.ingested_new;
            let ingested_updated = outcome.cycle.ingested_updated;
            let operator_report =
                scout_report::build_operator_report(&outcome, &outcome.scheduled_threats);
            let _ = tx.send(format!(
                "[WAR ROOM] {}",
                operator_report.executive_summary_ru
            ));
            store_bdu_cache(&state, &outcome, completed_at).await;
            if let Some(audit) = &state.audit {
                let _ = audit.log_event(
                    "scout_pipeline",
                    &format!(
                        "fstec found={} ingested={} new={} updated={} critic={} risk={:.2} fusion={} heal={}/{} deception={}",
                        outcome.vulns.len(),
                        ingested,
                        ingested_new,
                        ingested_updated,
                        outcome.cycle.critic_verdict,
                        outcome.cycle.critic_risk,
                        outcome.fusion_updated,
                        outcome.healing_applied,
                        outcome.healing_attempted,
                        outcome.deception_deployed
                    ),
                    outcome.cycle.critic_risk,
                    true,
                );
            }
            println!(
                "[API] /api/scout pipeline findings={} bdu={} sources_ok={} new={} updated={} critic={} fusion={} heal_q={}",
                outcome.findings.len(),
                outcome.vulns.len(),
                outcome.sources_ok,
                ingested_new,
                ingested_updated,
                outcome.cycle.critic_verdict,
                outcome.fusion_updated,
                outcome.healing_attempted
            );
            (
                StatusCode::OK,
                Json(ScoutResponse {
                    status: "success",
                    found: outcome.findings.len(),
                    ingested,
                    ingested_new,
                    ingested_updated,
                    source: "scout_2_multi",
                    completed_at,
                    items: outcome.vulns,
                    critic_verdict: outcome.cycle.critic_verdict,
                    critic_risk: outcome.cycle.critic_risk,
                    inquisitor_blocks: outcome.cycle.inquisitor_blocks,
                    inquisitor_escalates: outcome.cycle.inquisitor_escalates,
                    fusion_updated: outcome.fusion_updated,
                    deception_deployed: outcome.deception_deployed,
                    healing_attempted: outcome.healing_attempted,
                    healing_applied: outcome.healing_applied,
                    total_findings: outcome.findings.len(),
                    sources_ok: outcome.sources_ok,
                    sources_skipped: outcome.sources_skipped,
                    sources_failed: outcome.sources_failed,
                    sources: outcome
                        .source_statuses
                        .iter()
                        .map(|s| {
                            serde_json::json!({
                                "id": s.id,
                                "label": s.label,
                                "status": s.status,
                                "count": s.count,
                                "note": s.note,
                            })
                        })
                        .collect(),
                    enrichment_merged: outcome.enrichment_merged,
                    total_iocs: outcome.total_iocs,
                    total_cves: outcome.total_cves,
                    pipeline: vec![
                        "scout",
                        "enrichment",
                        "critic",
                        "inquisitor",
                        "ingest",
                        "fusion",
                        "self_healing_async",
                        "deception",
                        "war_room",
                    ],
                    report: Some(operator_report),
                    error: None,
                }),
            )
        }
        Err(e) => {
            let _ = tx.send(format!("[SCOUT] ✗ Pipeline error: {}", e));
            eprintln!("[API] /api/scout pipeline error: {}", e);
            let status = if e.contains("уже выполняется") {
                StatusCode::CONFLICT
            } else if e.contains("timeout") {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::BAD_GATEWAY
            };
            (status, Json(empty_scout_error(&e, completed_at)))
        }
    }
}

fn empty_scout_error(err: &str, completed_at: i64) -> ScoutResponse {
    ScoutResponse {
        status: "error",
        found: 0,
        ingested: 0,
        ingested_new: 0,
        ingested_updated: 0,
        source: "fstec_bdu",
        completed_at,
        items: vec![],
        critic_verdict: "—".into(),
        critic_risk: 0.0,
        inquisitor_blocks: 0,
        inquisitor_escalates: 0,
        fusion_updated: 0,
        deception_deployed: 0,
        healing_attempted: 0,
        healing_applied: 0,
        total_findings: 0,
        sources_ok: 0,
        sources_skipped: 0,
        sources_failed: 0,
        sources: vec![],
        enrichment_merged: 0,
        total_iocs: 0,
        total_cves: 0,
        pipeline: vec![],
        report: None,
        error: Some(err.to_string()),
    }
}

fn agent_data_root(state: &AppState) -> std::path::PathBuf {
    state
        .config
        .as_ref()
        .map(|c| {
            std::path::Path::new(&c.database.sqlite_path)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("./data"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("./data"))
}

#[derive(Deserialize)]
struct HealSandboxVerifyRequest {
    patch: Option<String>,
}

/// POST /api/heal/sandbox-verify — run real Docker sandbox validation (smoke / ops).
async fn api_heal_sandbox_verify(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    body: Option<Json<HealSandboxVerifyRequest>>,
) -> impl IntoResponse {
    let data_root = agent_data_root(&state);
    let patch_id = format!("sandbox-verify-{}", chrono::Utc::now().timestamp());
    let content = body
        .and_then(|Json(b)| b.patch)
        .unwrap_or_else(|| {
            format!(
                "# AEGIS sandbox verify\n# patch_id={}\nenabled=true\n",
                patch_id
            )
        });
    let executor = crate::sandbox_executor::SandboxExecutor::new(&data_root);
    let outcome = executor
        .test_patch(&patch_id, &content, "Low")
        .await;
    let status = if outcome.passed {
        "ok"
    } else {
        "failed"
    };
    (
        if outcome.passed {
            StatusCode::OK
        } else {
            StatusCode::UNPROCESSABLE_ENTITY
        },
        Json(serde_json::json!({
            "status": status,
            "passed": outcome.passed,
            "runtime": outcome.runtime,
            "duration_secs": outcome.duration_secs,
            "detail": outcome.detail,
            "patch_id": patch_id,
        })),
    )
        .into_response()
}

/// POST /api/heal/smoke — deterministic patch apply test (staging / smoke).
async fn api_heal_smoke(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    let data_root = agent_data_root(&state);
    let patch_id = format!("smoke-{}", chrono::Utc::now().timestamp());
    let content = format!(
        "# AEGIS smoke heal patch\n# patch_id={}\nenabled=true\n",
        patch_id
    );
    let executor = crate::sandbox_executor::SandboxExecutor::new(&data_root);
    let sandbox = executor
        .test_patch(&patch_id, &content, "Low")
        .await;
    if !sandbox.passed {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "status": "sandbox_failed",
                "patch_id": patch_id,
                "runtime": sandbox.runtime,
                "duration_secs": sandbox.duration_secs,
                "detail": sandbox.detail,
            })),
        )
            .into_response();
    }
    let enforce = crate::patch_applier::heal_apply_enforced();
    let applier = crate::patch_applier::PatchApplier::new(&data_root);
    let record = match applier.apply_config_patch(&patch_id, &content, enforce) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "error": e,
                })),
            )
                .into_response();
        }
    };
    let applied = enforce && record.mode == "applied";

    if let Some(audit) = &state.audit {
        let _ = audit.log_event(
            "heal_smoke",
            &format!(
                "patch_id={} mode={} applied={} path={}",
                record.patch_id, record.mode, applied, record.path
            ),
            0.1,
            applied,
        );
    }

    Json(serde_json::json!({
        "status": "ok",
        "patch_id": record.patch_id,
        "mode": record.mode,
        "applied": applied,
        "enforce": enforce,
        "path": record.path,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct DeceptionDeployRequest {
    port: Option<u16>,
    #[serde(rename = "type")]
    htype: Option<String>,
}

/// POST /api/deception/deploy — deploy Docker nginx deception listener.
async fn api_deception_deploy(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    body: Option<Json<DeceptionDeployRequest>>,
) -> impl IntoResponse {
    let Some(hp) = &state.honeypots else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "status": "error", "error": "honeypots not configured" })),
        )
            .into_response();
    };
    let htype = body
        .as_ref()
        .and_then(|Json(b)| b.htype.as_deref())
        .map(parse_honeypot_type)
        .unwrap_or(HoneypotType::WebAdmin);
    let port = body
        .and_then(|Json(b)| b.port)
        .unwrap_or(19_080);
    let id = match hp.spawn(htype.clone(), port).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "status": "error", "error": e })),
            )
                .into_response();
        }
    };
    let instances = hp.list_active().await;
    let inst = instances
        .iter()
        .find(|i| i.id == id)
        .cloned()
        .unwrap_or_else(|| instances.last().cloned().unwrap());
    let data_root = agent_data_root(&state);
    let deception = crate::deception_runtime::DeceptionRuntime::new(&data_root);
    let http_ok = deception
        .verify_local(inst.port, &inst.canary)
        .await;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "id": inst.id,
            "port": inst.port,
            "runtime": inst.runtime,
            "container_name": inst.container_name,
            "canary": inst.canary,
            "http_ok": http_ok,
        })),
    )
        .into_response()
}

#[derive(Deserialize)]
struct DeceptionCanaryTripRequest {
    token: String,
    source: Option<String>,
}

/// GET /api/heal/pending — HITL queue (sandbox-passed, awaiting human).
async fn api_heal_pending(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
) -> impl IntoResponse {
    let queue = crate::heal_queue::HealQueue::new(agent_data_root(&state));
    match queue.list_pending() {
        Ok(items) => Json(serde_json::json!({
            "status": "ok",
            "count": items.len(),
            "items": items,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "status": "error", "error": e })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct HealApproveRequest {
    patch_id: String,
    note: Option<String>,
}

/// POST /api/heal/approve — human approves pending patch → disk apply.
async fn api_heal_approve(
    State(state): State<AppState>,
    user: crate::auth::AuthUser,
    Json(req): Json<HealApproveRequest>,
) -> impl IntoResponse {
    if state.config.as_ref().is_some_and(|c| c.is_air_gapped()) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "status": "error",
                "error": "air_gapped: heal apply blocked",
            })),
        )
            .into_response();
    }
    let data_root = agent_data_root(&state);
    let queue = crate::heal_queue::HealQueue::new(&data_root);
    let Some(item) = queue.remove_pending(&req.patch_id).ok().flatten() else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "status": "error",
                "error": format!("pending patch not found: {}", req.patch_id),
            })),
        )
            .into_response();
    };
    let applier = crate::patch_applier::PatchApplier::new(&data_root);
    let _ = applier.prepare_snapshot();
    let record = match applier.apply_config_patch(&item.patch_id, &item.content, true) {
        Ok(r) => r,
        Err(e) => {
            let _ = queue.enqueue(item);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "status": "error", "error": e })),
            )
                .into_response();
        }
    };
    let risk_label = format!("{:?}", item.risk);
    crate::metrics::record_heal_hitl("approved", &risk_label);
    if let Some(audit) = &state.audit {
        let _ = audit.log_event(
            "heal_hitl",
            &format!(
                "approved id={} by={} note={} path={}",
                item.patch_id,
                user.0.sub,
                req.note.as_deref().unwrap_or("-"),
                record.path
            ),
            0.25,
            true,
        );
    }
    Json(serde_json::json!({
        "status": "ok",
        "patch_id": item.patch_id,
        "applied": true,
        "mode": record.mode,
        "path": record.path,
        "risk": risk_label,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct HealRejectRequest {
    patch_id: String,
    reason: Option<String>,
}

/// POST /api/heal/reject — remove from HITL queue + audit.
async fn api_heal_reject(
    State(state): State<AppState>,
    user: crate::auth::AuthUser,
    Json(req): Json<HealRejectRequest>,
) -> impl IntoResponse {
    let queue = crate::heal_queue::HealQueue::new(agent_data_root(&state));
    let Some(item) = queue.remove_pending(&req.patch_id).ok().flatten() else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "status": "error",
                "error": format!("pending patch not found: {}", req.patch_id),
            })),
        )
            .into_response();
    };
    let risk_label = format!("{:?}", item.risk);
    crate::metrics::record_heal_hitl("rejected", &risk_label);
    if let Some(audit) = &state.audit {
        let _ = audit.log_event(
            "heal_hitl",
            &format!(
                "rejected id={} by={} reason={}",
                item.patch_id,
                user.0.sub,
                req.reason.as_deref().unwrap_or("-")
            ),
            0.4,
            false,
        );
    }
    Json(serde_json::json!({
        "status": "ok",
        "patch_id": item.patch_id,
        "rejected": true,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct HealRunRequest {
    anomaly: String,
    #[serde(default)]
    patch_type: Option<String>,
}

fn parse_patch_type(s: &str) -> crate::healing_orchestrator::PatchType {
    match s.to_ascii_lowercase().as_str() {
        "config" => crate::healing_orchestrator::PatchType::Config,
        "code" => crate::healing_orchestrator::PatchType::Code,
        "dependency" => crate::healing_orchestrator::PatchType::Dependency,
        "isolation" => crate::healing_orchestrator::PatchType::Isolation,
        "custom" | "critical" => crate::healing_orchestrator::PatchType::Custom,
        _ => crate::healing_orchestrator::PatchType::Code,
    }
}

fn parse_honeypot_type(s: &str) -> HoneypotType {
    match s.to_ascii_lowercase().as_str() {
        "database" | "db" => HoneypotType::Database,
        "api" => HoneypotType::ApiEndpoint,
        "ssh" => HoneypotType::SshShell,
        "smb" | "share" => HoneypotType::WindowsShare,
        _ => HoneypotType::WebAdmin,
    }
}

/// POST /api/heal/run — full healing cycle (verify → sandbox → HITL or apply).
async fn api_heal_run(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<HealRunRequest>,
) -> impl IntoResponse {
    let anomaly = req.anomaly.trim();
    if anomaly.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "status": "error", "error": "anomaly required" })),
        )
            .into_response();
    }
    let Some(healing) = &state.healing else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "status": "error", "error": "healing not configured" })),
        )
            .into_response();
    };
    let patch_type = req
        .patch_type
        .as_deref()
        .map(parse_patch_type)
        .unwrap_or(crate::healing_orchestrator::PatchType::Code);
    match healing.heal(anomaly, patch_type).await {
        Ok(result) => Json(serde_json::json!({
            "status": "ok",
            "result": result,
            "pending_hitl": result.apply_mode == "pending_hitl",
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "status": "error", "error": e })),
        )
            .into_response(),
    }
}

/// POST /api/deception/canary-trip — simulate canary access (audit + auto-deploy hook).
async fn api_deception_canary_trip(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<DeceptionCanaryTripRequest>,
) -> impl IntoResponse {
    let Some(hp) = &state.honeypots else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "status": "error", "error": "honeypots not configured" })),
        )
            .into_response();
    };
    let source = req.source.as_deref().unwrap_or("smoke");
    if let Err(e) = hp.track_canary(&req.token, source).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "status": "error", "error": e })),
        )
            .into_response();
    }
    Json(serde_json::json!({
        "status": "ok",
        "token": req.token,
        "source": source,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct ContainRequest {
    cluster_id: String,
}

async fn api_contain(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<ContainRequest>,
) -> impl IntoResponse {
    let cluster_id = req.cluster_id.trim().to_string();
    if cluster_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "error",
                "message": "cluster_id must not be empty"
            })),
        )
            .into_response();
    }

    let severity = if let Some(fusion) = &state.fusion {
        fusion
            .get_fused_threats(200)
            .await
            .into_iter()
            .find(|t| t.cluster_id == cluster_id)
            .map(|t| t.severity)
            .unwrap_or(0.85)
    } else {
        0.85
    };

    let base = AdaptiveIsolation::for_workload(Workload::Sentinel);
    let iso = AdaptiveIsolation::escalate(&base, severity);
    let fusion_marked = if let Some(fusion) = &state.fusion {
        fusion.mark_contained(&cluster_id).await
    } else {
        false
    };

    let blocked_total = state.store.increment("threats_blocked").await;

    let data_root = agent_data_root(&state);
    let contain_record = crate::contain_enforcer::ContainEnforcer::new(&data_root)
        .enforce(&cluster_id, severity, &iso)
        .ok();

    if let Some(audit) = &state.audit {
        let _ = audit.log_event(
            "contain",
            &format!(
                "cluster={} severity={:.2} level={:?} runtime={} network={}",
                cluster_id,
                severity,
                iso.level,
                iso.runtime,
                network_policy_label(&iso.network)
            ),
            severity,
            true,
        );
    }

    let msg = format!(
        "[CONTAIN] cluster={} | severity={:.2} | isolation={:?}/{} | network={} | threats_blocked={}",
        cluster_id,
        severity,
        iso.level,
        iso.runtime,
        network_policy_label(&iso.network),
        blocked_total
    );
    let _ = state.alert_tx.send(msg);

    Json(serde_json::json!({
        "status": "contained",
        "cluster_id": cluster_id,
        "severity": severity,
        "isolation_level": format!("{:?}", iso.level),
        "runtime": iso.runtime,
        "network": network_policy_label(&iso.network),
        "threats_blocked": blocked_total,
        "fusion_marked": fusion_marked,
        "enforcement_mode": contain_record.as_ref().map(|r| r.enforcement_mode.clone()).unwrap_or_else(|| "policy".into()),
        "host_enforced": contain_record.as_ref().map(|r| r.host_enforced).unwrap_or(false),
        "contain_record": contain_record,
    }))
    .into_response()
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

#[derive(Deserialize)]
struct AirGapRequest {
    enabled: bool,
}

async fn api_air_gap(
    State(state): State<AppState>,
    _user: crate::auth::AuthUser,
    Json(req): Json<AirGapRequest>,
) -> impl IntoResponse {
    let mut ag = state.air_gapped.lock().await;
    *ag = req.enabled;
    
    let status_msg = if req.enabled {
        "ВНИМАНИЕ: Система переведена в изолированный режим (Air-Gapped). Внешние связи разорваны."
    } else {
        "Система выведена из изолированного режима. Внешние связи восстановлены."
    };
    let _ = state.alert_tx.send(status_msg.to_string());
    
    Json(serde_json::json!({ "success": true, "air_gapped": *ag }))
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
    const RL_STATUS_PUBLIC: RateLimitRule = RateLimitRule {
        name: "api_status_public",
        max_requests: 30,
        window: std::time::Duration::from_secs(60),
    };
    const RL_METRICS: RateLimitRule = RateLimitRule {
        name: "metrics",
        max_requests: 60,
        window: std::time::Duration::from_secs(60),
    };

    let is_dev = std::env::var("AEGIS_DEV_MODE").map(|v| v == "1").unwrap_or(false);

    let fed_auth = match state.config.as_ref() {
        Some(cfg) => crate::federation_auth::FederationAuthState::from_federation(
            &cfg.federation,
            &cfg.mode,
        ),
        None => crate::federation_auth::FederationAuthState::from_secret(None),
    };

    // === ЗАЩИЩЁННЫЕ FEDERATION РОУТЫ (peer token; optional mTLS on outbound client) ===
    let federation = Router::new()
        .route("/federation/merkle", get(federation_merkle_handler))
        .route("/federation/hashes", get(federation_hashes_handler))
        .route("/federation/items", get(federation_items_handler))
        .route("/federation/changed_since", axum::routing::post(federation_changed_since_handler))
        .route("/federation/missing", axum::routing::post(federation_missing_handler))
        .route("/federation/receive", axum::routing::post(federation_receive_handler))
        .route_layer(from_fn_with_state(
            fed_auth,
            crate::federation_auth::federation_peer_auth_middleware,
        ));

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
            "/api/status/public",
            get(api_status_public).route_layer(from_fn_with_state(
                RateLimitState {
                    limiter: state.rate_limiter.clone(),
                    rule: RL_STATUS_PUBLIC,
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
        .route("/api/audit-tail", get(api_audit_tail))
        .route("/api/fused-threats", get(api_fused_threats))
        .route("/api/federation/nodes", get(api_federation_nodes))
        .route("/api/federation/health", get(api_federation_health))
        .route("/api/federation/metrics", get(api_federation_metrics))
        .route("/api/federation/merkle", get(api_federation_merkle))
        .route("/api/raft/status", get(api_raft_status))
        .route("/api/raft/metrics", get(api_raft_metrics))
        .route("/api/react/status", get(api_react_status))
        .route("/api/federation/sync", axum::routing::post(api_federation_sync))
        .route("/api/air-gap", axum::routing::post(api_air_gap))
        .route("/api/agents/:id/:action", axum::routing::post(api_agents_action))
        .route("/api/agents", get(api_agents))
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
        .route("/api/bdu/recent", get(api_bdu_recent))
        .route("/api/heal/smoke", axum::routing::post(api_heal_smoke))
        .route(
            "/api/heal/sandbox-verify",
            axum::routing::post(api_heal_sandbox_verify),
        )
        .route(
            "/api/deception/deploy",
            axum::routing::post(api_deception_deploy),
        )
        .route(
            "/api/deception/canary-trip",
            axum::routing::post(api_deception_canary_trip),
        )
        .route("/api/heal/pending", get(api_heal_pending))
        .route("/api/heal/approve", axum::routing::post(api_heal_approve))
        .route("/api/heal/reject", axum::routing::post(api_heal_reject))
        .route("/api/heal/run", axum::routing::post(api_heal_run))
        .route("/api/contain", axum::routing::post(api_contain))
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
    println!("🌐 AEGIS API Server on http://127.0.0.1:{}", port);
    println!("   /api/login — получить JWT (access + refresh)");
    println!("   /api/refresh — обновить access token");
    println!("   /api/status, /api/knowledge, /api/threats, /api/fused-threats, /api/react, /api/scout, /api/ingest* — ЗАЩИЩЕНЫ JWT");
    let bind_addr = format!("127.0.0.1:{}", port);
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
        Ok(root) => Json(serde_json::json!({
            "merkle_root": root,
            "root": root
        }))
        .into_response(),
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

async fn federation_hashes_handler(
    State(state): State<AppState>,
) -> axum::response::Response {
    let federation = match state.federation.as_ref() {
        Some(f) => f,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "FederationLayer is not initialized in AppState" })),
            )
                .into_response();
        }
    };
    match federation.kb.get_all_hashes().await {
        Ok(rows) => {
            let hashes: Vec<String> = rows
                .into_iter()
                .map(|(_, h)| h)
                .filter(|h| !h.is_empty())
                .collect();
            let count = hashes.len();
            Json(serde_json::json!({ "hashes": hashes, "count": count })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to get hashes: {}", e) })),
        )
            .into_response(),
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

    let their_set: std::collections::HashSet<String> = their_hashes.into_iter().collect();
    let my_hashes = match federation.kb.get_all_hashes().await {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": format!("Failed to get hashes: {}", e) }))).into_response(),
    };

    let mut missing = Vec::new();
    for (_, hash) in my_hashes {
        if hash.is_empty() || their_set.contains(&hash) {
            continue;
        }
        if let Ok(Some(item)) = federation.kb.get_by_content_hash(&hash).await {
            missing.push(item);
        }
    }

    Json(missing).into_response()
}

async fn federation_receive_handler(
    State(state): State<AppState>,
    Json(items): Json<Vec<crate::knowledge_item::KnowledgeItem>>,
) -> axum::response::Response {
    let federation = match state.federation.as_ref() {
        Some(f) => f,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "FederationLayer is not initialized in AppState" })),
            )
                .into_response();
        }
    };
    let mut accepted = 0usize;
    for item in items {
        if federation.ingest_federated_item(item).await.is_ok() {
            accepted += 1;
        }
    }
    Json(serde_json::json!({ "accepted": accepted })).into_response()
}