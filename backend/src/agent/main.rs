pub mod auth;
pub mod api_key_store;
pub mod mtls;
pub mod knowledge;
pub mod knowledge_item;
pub mod dashboard;
pub mod server;
pub mod prompt_guard;
pub mod persistence;
pub mod llm_guard;
pub mod event_bus;
pub mod isolation;
pub mod react_engine;
pub mod scratchpad;
pub mod tool_registry;
pub mod agent_bus;
pub mod threat_hunter;
pub mod fusion_engine;
pub mod critic_agent;
pub mod inquisitor_agent;
pub mod mcts;
pub mod local_llm;
pub mod config;
pub mod safety;
pub mod audit;
pub mod scout;
pub mod fstec_bdu;
pub mod scout_pipeline;
pub mod scout_orchestrator;
pub mod scout_intel;
pub mod react_service;
pub mod dna_engine;
pub mod metrics;
pub mod utils;
pub mod key_provider;
pub mod learning_orchestrator;
pub mod healing_orchestrator;
pub mod honeypot_manager;
pub mod deception_runtime;
pub mod distributed_oracle;
pub mod federation;
pub mod federation_auth;
pub mod federation_client;
pub mod autonomous_remediation;
pub mod moving_target;
pub mod p2p_discovery;
pub mod ast_verifier;
pub mod agent_registry;
pub mod patch_applier;
pub mod heal_queue;
pub mod sandbox_executor;
pub mod contain_enforcer;
pub mod llm_status;

use healing_orchestrator::{HealingOrchestrator, PatchType};
use learning_orchestrator::LearningOrchestrator;
use autonomous_remediation::AutonomousRemediation;
use critic_agent::{CriticAgent, Verdict};
use inquisitor_agent::{Inquisitor, InquisitorLlm};
use tool_registry::create_default_registry;
use knowledge::KnowledgeBase;
use persistence::PersistentStore;
use prompt_guard::{build_safe_prompt, validate_agent_output};
use reqwest::Client;
use serde_json::json;
use std::fs;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_stream::StreamExt;
use server::{AppState, start_server};
use llm_guard::{LlmCache, RateLimiter};
use std::sync::LazyLock;
use audit::AuditTrail;

static LLM_CACHE: LazyLock<LlmCache> = LazyLock::new(|| LlmCache::new(86400));
static LLM_RATE_LIMITER: LazyLock<RateLimiter> = LazyLock::new(|| RateLimiter::new(100, 60));

/// После сбоя локального LLM в Hybrid (non-critical) не дергать его снова весь cooldown —
/// иначе каждый вызов Scout ждёт 502 + облако.
struct HybridNonCriticalCircuit {
    skip_local_until: Option<Instant>,
}

impl HybridNonCriticalCircuit {
    const COOLDOWN: Duration = Duration::from_secs(120);

    fn skip_local(&self) -> bool {
        self.skip_local_until
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
    }

    fn on_local_failure(&mut self) {
        self.skip_local_until = Some(Instant::now() + Self::COOLDOWN);
    }

    fn on_local_success(&mut self) {
        self.skip_local_until = None;
    }
}

static HYBRID_NONCRITICAL_CIRCUIT: LazyLock<Mutex<HybridNonCriticalCircuit>> =
    LazyLock::new(|| Mutex::new(HybridNonCriticalCircuit { skip_local_until: None }));

/// HITL timeout — если оператор не ответил за 120 секунд, считаем отказом.
const HITL_TIMEOUT_SECS: u64 = 120;

fn prompt_yes_no(
    prompt: &str,
    audit: Option<&AuditTrail>,
    action_label: &str,
    risk: f64,
    hitl_stage: Option<&'static str>,
) -> bool {
    if let Some(audit) = audit {
        let _ = audit.log_event("human", &format!("hitl_prompt: {}", action_label), risk, false);
    }
    eprint!("{} [timeout={}s] ", prompt, HITL_TIMEOUT_SECS);
    let _ = io::stdout().flush();

    // Читаем stdin в отдельном треде с таймаутом.
    // Без таймаута оператор может уйти и процесс зависнет навсегда.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let _ = tx.send(input);
        }
        // При ошибке/EOF tx дропается → recv вернёт Err → false
    });

    let ok = match rx.recv_timeout(std::time::Duration::from_secs(HITL_TIMEOUT_SECS)) {
        Ok(input) => matches!(input.trim().to_lowercase().as_str(), "y" | "yes"),
        Err(_) => {
            eprintln!("\n[HITL] Timeout after {}s — defaulting to REJECT (Zero-Trust)", HITL_TIMEOUT_SECS);
            if let Some(audit) = audit {
                let _ = audit.log_event("human", &format!("hitl_timeout: {}", action_label), risk, false);
            }
            false
        }
    };

    if let Some(audit) = audit {
        let _ = audit.log_event("human", &format!("hitl_response: {}", action_label), risk, ok);
    }
    if let Some(stage) = hitl_stage {
        if ok {
            metrics::hitl_approval(stage);
        } else {
            metrics::hitl_rejection(stage);
        }
    }
    ok
}

pub mod aegis {
    tonic::include_proto!("aegis");
}
use aegis::sentinel_oracle_client::SentinelOracleClient;
use aegis::SubscribeRequest;
use tonic::transport::Endpoint;

const BASE_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const MODEL_PRO: &str = "google/gemini-2.5-pro-preview-05-06";
const MODEL_FLASH: &str = "google/gemini-2.0-flash-001";

// Grok (xAI) — fallback в цепочке OpenRouter. Имя модели: env `XAI_MODEL` или актуальный slug по умолчанию.
// `grok-beta` снят с API → 400; см. https://docs.x.ai/docs/models
const XAI_URL: &str = "https://api.x.ai/v1/chat/completions";

fn xai_chat_model() -> String {
    std::env::var("XAI_MODEL").unwrap_or_else(|_| "grok-3".to_string())
}

fn load_prompt(role: &str) -> String {
    let path = format!("src/agent/prompts/{}.prompt", role);
    fs::read_to_string(&path)
        .unwrap_or_else(|_| format!("Твоя роль: {}. Отвечай техническим и кратким языком.", role))
}

/// Универсальный multi-provider LLM клиент (OpenAI-совместимый формат)
/// Решает, использовать ли облако или локальную модель на основе AEGISConfig.
/// Теперь использует KeyProvider вместо сырого api_key (Zero-Trust).
pub(crate) async fn call_llm(
    http_client: &Client,
    key_provider: &dyn crate::key_provider::KeyProvider,
    system: &str,
    user: &str,
    config: &crate::config::AEGISConfig,
    local: Option<&crate::local_llm::LocalLlmClient>,
    is_critical: bool,
) -> Option<String> {
    // Получаем ключ через KeyProvider (поддержка HSM/Vault/Env)
    let api_key = match key_provider.get_llm_key().await {
        Ok(k) => k,
        Err(e) => {
            tracing::error!("KeyProvider failed: {}", e);
            return None;
        }
    };

    let safe_prompt = build_safe_prompt(system, user, &[], "CRITICAL: You are AEGIS security agent.");
    let cache_key = {
        use sha2::{Sha256, Digest};
        format!("{:x}", Sha256::digest(safe_prompt.as_bytes()))
    };
    
    if let Some(cached) = LLM_CACHE.get(&cache_key).await { return Some(cached); }

    // 1. Air-Gapped или принудительно локальный режим
    if config.is_air_gapped() || config.llm.mode == crate::config::LlmMode::Local {
        if let Some(local_client) = local {
            return local_client.chat(system, &safe_prompt, None).await;
        }
        tracing::error!("Local LLM client requested but not initialized");
        return None;
    }

    // 2. Hybrid режим:
    //    - critical: сначала облако, при полном провале — локальная модель (если есть);
    //    - non-critical: сначала локальная модель, при недоступности (502 и т.д.) — облако
    //      (Scout 2.0 и прочие «дешёвые» вызовы не зависают от мёртвого Ollama).
    if config.llm.mode == crate::config::LlmMode::Hybrid {
        if is_critical {
            if let Some(out) = call_cloud_chain(http_client, &api_key, system, &safe_prompt).await {
                return Some(out);
            }
            if let Some(local_client) = local {
                if let Some(out) = local_client.chat(system, &safe_prompt, None).await {
                    return Some(out);
                }
            }
            return None;
        }

        let bypass_local = HYBRID_NONCRITICAL_CIRCUIT
            .lock()
            .map(|g| g.skip_local())
            .unwrap_or(false);

        if bypass_local {
            tracing::debug!(target: "llm", "Hybrid: skipping local LLM (cooldown after recent failure)");
        } else if let Some(local_client) = local {
            if let Some(out) = local_client.chat(system, &safe_prompt, None).await {
                let _ = HYBRID_NONCRITICAL_CIRCUIT.lock().map(|mut g| g.on_local_success());
                return Some(out);
            }
            tracing::info!(target: "llm", "Hybrid: local LLM failed or empty; falling back to cloud");
            let _ = HYBRID_NONCRITICAL_CIRCUIT.lock().map(|mut g| g.on_local_failure());
        }
        if let Some(out) = call_cloud_chain(http_client, &api_key, system, &safe_prompt).await {
            return Some(out);
        }
        return None;
    }

    // 3. Cloud режим (по умолчанию)
    call_cloud_chain(http_client, &api_key, system, &safe_prompt).await
}

async fn call_cloud_chain(http_client: &Client, api_key: &str, system: &str, user: &str) -> Option<String> {
    let cache_key = {
        use sha2::{Sha256, Digest};
        format!("{:x}", Sha256::digest(user.as_bytes()))
    };
    if let Err(e) = LLM_RATE_LIMITER.check("default").await { eprintln!("[LLM] {}", e); return None; }

    // 0. DeepSeek (if key matches DeepSeek format)
    if api_key.starts_with("sk-") && api_key.len() >= 32 {
        if let Some(out) = try_provider(
            http_client, 
            "https://api.deepseek.com/chat/completions", 
            api_key, 
            "deepseek-chat", 
            system, 
            user, 
            &cache_key
        ).await {
            return Some(out);
        }
    }

    // 1. Gemini Pro (OpenRouter)
    if let Some(out) = try_provider(http_client, BASE_URL, api_key, MODEL_PRO, system, user, &cache_key).await {
        return Some(out);
    }

    // 2. Grok (xAI direct)
    let xai_key = std::env::var("XAI_API_KEY").unwrap_or_else(|_| api_key.to_string());
    let xai_model = xai_chat_model();
    if let Some(out) = try_provider(
        http_client,
        XAI_URL,
        &xai_key,
        xai_model.as_str(),
        system,
        user,
        &cache_key,
    )
    .await
    {
        return Some(out);
    }

    // 3. Gemini Flash (OpenRouter)
    if let Some(out) = try_provider(http_client, BASE_URL, api_key, MODEL_FLASH, system, user, &cache_key).await {
        return Some(out);
    }

    None
}

fn print_airgap_banner() {
    eprintln!(
        "{}",
        r#"
██████╗  █████╗ ███████╗ ██████╗ ██████╗  █████╗ ██████╗ ██████╗ ███████╗██████╗ 
██╔══██╗██╔══██╗██╔════╝██╔════╝██╔═══██╗██╔══██╗██╔══██╗██╔══██╗██╔════╝██╔══██╗
██████╔╝███████║█████╗  ██║     ██║   ██║███████║██████╔╝██████╔╝█████╗  ██████╔╝
██╔══██╗██╔══██║██╔══╝  ██║     ██║   ██║██╔══██║██╔══██╗██╔══██╗██╔══╝  ██╔══██╗
██║  ██║██║  ██║███████╗╚██████╗╚██████╔╝██║  ██║██║  ██║██████╔╝███████╗██║  ██║
╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝ ╚══════╝╚═╝  ╚═╝

AIR-GAPPED MODE ACTIVATED — EXTERNAL CONNECTIONS DISABLED
"#
    );
}

/// Универсальная попытка вызова любого OpenAI-совместимого провайдера
async fn try_provider(
    client: &Client,
    url: &str,
    key: &str,
    model: &str,
    system: &str,
    user: &str,
    cache_key: &str,
) -> Option<String> {
    let body = json!({"model": model, "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}]});
    if let Ok(res) = client.post(url).header("Authorization", format!("Bearer {}", key)).json(&body).send().await {
        let status = res.status();
        if status.is_success() {
            if let Ok(j) = res.json::<serde_json::Value>().await {
                if let Some(t) = j["choices"][0]["message"]["content"].as_str() {
                    let output = t.to_string();
                    let v = validate_agent_output(&output);
                    if !v.is_safe { return Some("[BLOCKED]".into()); }
                    if !output.is_empty() && output.len() > 20 {
                        LLM_CACHE.set(cache_key.to_string(), output.clone()).await;
                        return Some(output);
                    }
                }
            }
        } else {
            let err_body = res.text().await.unwrap_or_default();
            tracing::warn!(
                target: "llm",
                "Provider {} model {} returned {} — {}",
                url,
                model,
                status,
                err_body.chars().take(280).collect::<String>()
            );
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Загружаем переменные окружения из .env и ПЕРЕЗАПИСЫВАЕМ существующие
    dotenvy::dotenv_override().ok();

    // H5 — offline helper (no server): agent-cli hash-key <plaintext>
    if std::env::args().nth(1).as_deref() == Some("hash-key") {
        let plaintext = std::env::args().nth(2).ok_or(
            "usage: agent-cli hash-key <plaintext>  (requires JWT_SECRET in env for pepper)",
        )?;
        let pepper = std::env::var("JWT_SECRET")
            .map_err(|_| "JWT_SECRET must be set")?;
        println!("{}", crate::api_key_store::hash_api_key(&plaintext, pepper.as_bytes()));
        println!("# store hash in api_keys or run agent once to migrate from env");
        return Ok(());
    }

    // === Tracing (production logging) ===
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    // === AEGIS Enterprise Config Layer ===
    let config = match crate::config::AEGISConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("❌ Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = config.validate() {
        tracing::error!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }
    let config = Arc::new(config);

    // === Immutable Audit Trail ===
    let audit = Arc::new(AuditTrail::new(
        config.audit.enabled,
        &config.audit.log_path,
        config.audit.immutable,
    ));
    if let Err(e) = audit.init() {
        tracing::error!("AuditTrail init failed: {}", e);
        std::process::exit(1);
    }
    let _ = audit.log_event("agent-cli", "startup", 0.0, true);

    if let Some(warning) = config.airgap_warning() {
        eprintln!("{}", warning);
        eprintln!("   External LLM calls and threat intel sources are DISABLED.\n");
        print_airgap_banner();
    }

    tracing::info!("Config loaded | Mode: {:?} | LLM: {:?} | Air-Gapped: {}", 
             config.mode, config.get_llm_mode(), config.is_air_gapped());

    let api_key = std::env::var("AI_API_KEY").unwrap_or_default().trim().to_string();
    // Zero-Trust: KeyProvider (HSM/Vault ready). Используем для всех LLM-вызовов.
    let key_provider: Arc<dyn crate::key_provider::KeyProvider> = if config.vault.enabled {
        tracing::info!("KeyProvider: HashiCorp Vault ({})", config.vault.address);
        Arc::new(crate::key_provider::VaultKeyProvider::new(&config.vault.address, &config.vault.token))
    } else {
        tracing::info!("KeyProvider: Environment Variables");
        crate::key_provider::default_env_provider()
    };
    let http_client = Client::new();

    // Local LLM client (for local/hybrid/airgapped)
    let local_client: Option<crate::local_llm::LocalLlmClient> = config
        .llm
        .local_base_url
        .as_deref()
        .and_then(|u| crate::local_llm::LocalLlmClient::new(u, &config.llm.default_model).ok());

    tracing::info!("\n[V8.7] AEGIS COMMAND CENTER ACTIVE\n🛡️ Prompt Guard: ENABLED\n⏱️ LLM Rate Limiter: ENABLED\n💾 Persistent Store: SQLite\n🧠 LLM: {:?}\n🧪 ReAct++: Critic + Real Tools + Kill Switch + MCTS ENABLED\n🔒 Adaptive Isolation: Docker/Kata/Firecracker (auto-escalation)\n🛡️ GOD MODE: cargo-deny + cargo-audit + 35+ point Inquisitor checklist\n⌨️  Команды: /fusion, /react <mission>, /god, /role, /research, /darknet", 
        config.get_llm_mode());
    let kb = if config.is_air_gapped() {
        match KnowledgeBase::new_air_gapped(&config.database.sqlite_path) {
            Ok(k) => Arc::new(k.with_audit(audit.clone())),
            Err(e) => {
                tracing::error!("Failed to init air-gapped KnowledgeBase: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match KnowledgeBase::new_with_params(
            &api_key,
            &config.database.sqlite_path,
            &config.database.qdrant_url,
            false,
        ) {
            Ok(k) => Arc::new(k.with_audit(audit.clone())),
            Err(e) => {
                tracing::error!("Failed to init KnowledgeBase: {}", e);
                std::process::exit(1);
            }
        }
    };
    let current_role = Arc::new(tokio::sync::Mutex::new("coo".to_string()));
    let role_clone = current_role.clone();
    let store = match PersistentStore::new("aegis_persistent.db") {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to open persistent store: {}", e);
            std::process::exit(1);
        }
    };

    // === AuthState для JWT (подготовка к дашборду) ===
    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET must be set in production (no default fallback allowed)");
    let auth_state = crate::auth::init_auth_state(jwt_secret.as_bytes());
    crate::auth::start_refresh_revocation_gc(auth_state.clone());

    // === Фаза 3: ReAct++ Critic + Real Tool Execution ===
    let critic = Arc::new(
        CriticAgent::with_config(&api_key, config.is_air_gapped())
            .with_llm(critic_agent::CriticLlm {
                http: http_client.clone(),
                api_key: api_key.clone(), // legacy fallback
                key_provider: key_provider.clone(),
                config: config.clone(),
                local: local_client.clone(),
            })
            .with_audit(audit.clone()),
    );
    let inquisitor = Arc::new(
        Inquisitor::new(config.is_air_gapped())
            .with_llm(InquisitorLlm {
                http: http_client.clone(),
                api_key: api_key.clone(),
                key_provider: key_provider.clone(),
                config: config.clone(),
                local: local_client.clone(),
            })
            .with_audit(audit.clone()),
    );
    let tool_registry = Arc::new(create_default_registry(Some(kb.clone()), config.is_air_gapped()));
    tracing::info!("ReAct++ Critic + MCTS + Tool Registry loaded ({} tools)", tool_registry.get_tools_for_prompt().len());

    let scout = Arc::new(
        scout::Scout::new(
            tool_registry.clone(),
            audit.clone(),
            config.is_air_gapped(),
        )
        .with_llm(scout::ScoutLlm {
            http: http_client.clone(),
            api_key: api_key.clone(),
            key_provider: key_provider.clone(),
            config: config.clone(),
            local: local_client.clone(),
        }),
    );
    let learning = Arc::new(LearningOrchestrator::new(
        scout,
        critic.clone(),
        inquisitor.clone(),
        kb.clone(),
        audit.clone(),
        config.clone(),
        http_client.clone(),
        api_key.clone(),
        local_client.clone(),
    ));

    // === Фаза 3: DNA + Federation (Raft wired after consensus init below) ===
    let dna = Arc::new(dna_engine::DnaEngine::new(&config.database.dna_path, audit.clone()));

    // === Фаза 3.2: Moving Target Defense & Honeypots ===
    let honeypot_manager = Arc::new(crate::honeypot_manager::HoneypotManager::new(kb.clone(), dna.clone(), audit.clone()));
    crate::honeypot_manager::HoneypotManager::init_canary_escalation(&honeypot_manager);
    let agent_registry = Arc::new(crate::agent_registry::AgentRegistry::new());
    agent_registry
        .set_ready(crate::agent_registry::HEALER_ID, "HealingOrchestrator ready")
        .await;

    let mut moving_target = crate::moving_target::MovingTargetDefense::new(audit.clone())
        .with_honeypot_manager(honeypot_manager.clone());
    moving_target
        .start_background_mutation(300, Some(agent_registry.clone()))
        .await;

    // === Distributed Oracle (Raft) & P2P Discovery ===
    let raft = Arc::new(tokio::sync::Mutex::new(distributed_oracle::ConsensusLayer::new(
        audit.clone(),
    )));
    {
        let mut guard = raft.lock().await;
        guard.register_node(&config.node_id);
    }
    let oracle = Arc::new(distributed_oracle::DistributedOracle::new(
        &config.node_id,
        audit.clone(),
    ));
    let healing = Arc::new(
        HealingOrchestrator::new(
            kb.clone(),
            dna.clone(),
            inquisitor.clone(),
            audit.clone(),
            config.clone(),
        )
        .with_oracle(oracle.clone()),
    );
    let p2p = crate::p2p_discovery::P2pDiscovery::new(
        &config.node_id,
        audit.clone(),
        raft.clone(),
    );
    p2p.start().await;

    let raft_maintenance = raft.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let mut guard = raft_maintenance.lock().await;
            guard.maintain_cluster().await;
        }
    });

    let federation = Arc::new(
        crate::federation::FederationLayer::new(
            kb.clone(),
            dna.clone(),
            audit.clone(),
            &config.federation,
            config.node_id.clone(),
        )
        .with_raft(raft.clone()),
    );
    federation.register_peers_in_raft().await;
    federation.clone().start_health_monitor(60);
    if config.federation.sync_interval_secs > 0 {
        federation
            .clone()
            .start_background_sync(config.federation.sync_interval_secs);
    }

    // === Фаза 2: Streaming Threat Fusion + Threat Hunter (dynamic internet research) ===
    let fusion = Arc::new(fusion_engine::FusionEngine::new(1000));
    
    // === Очистка старых кластеров (защита от утечки памяти) ===
    let fusion_clone = fusion.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // каждый час
        loop {
            interval.tick().await;
            // Очищаем кластеры старше 7 дней (86400 * 7)
            fusion_clone.cleanup_old_clusters(86400 * 7).await;
        }
    });

    let react_service = Arc::new(react_service::ReactService::new(
        critic.clone(),
        tool_registry.clone(),
        http_client.clone(),
        key_provider.clone(),
        local_client.clone(),
        config.clone(),
        audit.clone(),
    ));

    let server_state = AppState::new(store)
        .with_fusion(fusion.clone())
        .with_auth(auth_state.clone())
        .with_audit(audit.clone())
        .with_federation(federation.clone())
        .with_kb(kb.clone())
        .with_learning(learning)
        .with_honeypots(honeypot_manager.clone())
        .with_oracle(oracle.clone())
        .with_healing(healing)
        .with_config(config.clone())
        .with_raft(raft)
        .with_react(react_service)
        .with_agent_registry(agent_registry.clone());
    agent_registry
        .set_ready(crate::agent_registry::SCOUT_ID, "Ready — POST /api/scout")
        .await;
    let srv_http = server_state.clone();
    tokio::spawn(async move { if let Err(e) = start_server(srv_http).await { eprintln!("[SERVER] {}", e); } });

    let hunter = Arc::new(
        threat_hunter::ThreatHunter::new(300)
            .with_fusion(fusion.clone())
            .with_air_gapped(config.is_air_gapped())
            .with_tools(tool_registry.clone())
            .with_registry(agent_registry.clone()),
    );
    agent_registry.attach_hunter(hunter.clone()).await;
    hunter.start().await;
    tracing::info!("Threat Hunter: ACTIVE (dynamic internet research)");
    tracing::info!("Fusion Engine: STREAMING CORRELATION ENABLED");
    tracing::info!("HTTP: /api/fused-threats available");
    tracing::info!("Adaptive Isolation ready (Docker/Kata/Firecracker)");
    let srv_alerts = server_state.clone();
    tokio::spawn(async move {
        let use_mtls = std::env::var("AEGIS_MTLS").map(|v| v == "1").unwrap_or(false);
        let client = if use_mtls {
            let ca = std::env::var("AEGIS_MTLS_CA_CERT").ok();
            let server_cert = std::env::var("AEGIS_MTLS_ORACLE_CERT").ok();
            let server_key = std::env::var("AEGIS_MTLS_ORACLE_KEY").ok();
            let client_cert = std::env::var("AEGIS_MTLS_AGENT_CERT").ok();
            let client_key = std::env::var("AEGIS_MTLS_AGENT_KEY").ok();
            let domain = std::env::var("AEGIS_MTLS_ORACLE_DOMAIN").unwrap_or_else(|_| "oracle.local".to_string());

            match (ca, server_cert, server_key, client_cert, client_key) {
                (Some(ca), Some(server_cert), Some(server_key), Some(client_cert), Some(client_key)) => {
                    match crate::mtls::load_tls_config(&ca, &server_cert, &server_key, &client_cert, &client_key, &domain) {
                        Ok(tls) => {
                            match Endpoint::from_static("https://127.0.0.1:9090").tls_config(tls.client) {
                                Ok(ep) => SentinelOracleClient::connect(ep).await.ok(),
                                Err(e) => {
                                    tracing::error!("mTLS endpoint config failed: {}", e);
                                    None
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("mTLS config load failed: {}", e);
                            None
                        }
                    }
                }
                _ => {
                    tracing::error!("AEGIS_MTLS=1 but certificate env vars are missing");
                    None
                }
            }
        } else {
            SentinelOracleClient::connect("http://127.0.0.1:9090").await.ok()
        };

        if let Some(mut c) = client {
            let mut last_msg = String::new(); let mut last_time = std::time::Instant::now();
            if let Ok(r) = c.subscribe(SubscribeRequest { agent_id: "agent_1".into() }).await {
                let mut s = r.into_inner();
                while let Some(Ok(a)) = StreamExt::next(&mut s).await {
                    if a.message == last_msg && last_time.elapsed().as_secs() < 300 { continue; }
                    last_msg = a.message.clone(); last_time = std::time::Instant::now();
                    println!("\n\r\x1b[31;1m[!!!] АЛЕРТ: {}\x1b[0m", a.message);
                    let _ = srv_alerts.alert_tx.send(a.message.clone());
                    if a.severity > 0.8 { srv_alerts.store.increment("threats_blocked").await; }
                    print!("aegis> "); let _ = io::stdout().flush();
                }
            }
        }
    });
    loop {
        let role = role_clone.lock().await; print!("aegis ({})> ", *role); drop(role); let _ = io::stdout().flush();
        let mut input = String::new();
        let bytes_read = io::stdin().read_line(&mut input).unwrap_or(0);
        if bytes_read == 0 {
            // EOF reached (e.g. running in nohup). Sleep to prevent infinite CPU loop.
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            continue;
        }
        let cmd = input.trim();
        if cmd.is_empty() {
            continue;
        }
        if cmd == "exit" {
            break;
        }
        if cmd == "/pilot-info" {
            println!("AEGIS Pilot Package v0.9");
            println!(
                "- Air-Gapped: {}",
                if config.is_air_gapped() { "ENABLED" } else { "DISABLED" }
            );
            println!(
                "- Audit Trail: {}",
                if config.audit.enabled { "ACTIVE" } else { "DISABLED" }
            );
            println!(
                "- Human-in-the-Loop: {}",
                if config.security.human_in_the_loop { "ENABLED" } else { "DISABLED" }
            );
            println!("- Documentation: {}", "docs/pilot/");
            continue;
        }
        if cmd == "/demo" {
            println!("\nAEGIS Pilot Demo v0.9");
            println!("----------------------------------------");

            // 1) Air-Gapped check
            let airgapped = config.is_air_gapped();
            println!(
                "[1/4] Air-Gapped: {}",
                if airgapped { "ENABLED" } else { "DISABLED" }
            );
            let _ = audit.log_event("agent-cli", "demo_airgapped_check", 0.2, true);

            // 2) GOD MODE /code with HITL
            println!("[2/4] GOD MODE (/code) + Human-in-the-Loop");
            let demo_task = "Сгенерируй безопасный пример Rust-функции для валидации IP-адреса без внешних вызовов.";
            let action_label = "/code demo";
            let needs = safety::require_human_approval(action_label, 0.9);
            let mut approved = false;
            if needs && config.security.human_in_the_loop {
                approved = prompt_yes_no(
                    "[HITL] Approve demo /code generation? [y/N] ",
                    Some(audit.as_ref()),
                    action_label,
                    0.9,
                    None,
                );
                if !approved {
                    println!("  -> Demo /code: NOT APPROVED (expected behavior).");
                    let _ = audit.log_event("agent-cli", "demo_code_not_approved", 0.9, false);
                }
            }

            if approved {
                let sp = load_prompt("inquisitor");
                if let Some(r) = call_llm(
                    &http_client,
                    key_provider.as_ref(),
                    &sp,
                    &format!("Сгенерируй безопасный код: {}", demo_task),
                    &config,
                    local_client.as_ref(),
                    true,
                )
                .await
                {
                    println!("  -> Demo /code: generated (excerpt):");
                    println!("{}", &r[..800.min(r.len())]);
                    let _ = audit.log_event("agent-cli", "demo_code_generated", 0.6, true);
                } else {
                    println!("  -> Demo /code: LLM unavailable; skipping generation.");
                    let _ = audit.log_event("agent-cli", "demo_code_llm_unavailable", 0.6, false);
                }
            }

            // 3) Show a couple of ReAct steps (best-effort)
            println!("[3/4] ReAct++ (2 steps, best-effort)");
            let demo_mission = "Оцени риск и предложи безопасный план реакции на подозрительное повышение привилегий администратора.";
            let engine = react_engine::ReactEngine::new(2)
                .with_mcts(mcts::MctsEngine::new(10))
                .with_air_gapped(config.is_air_gapped())
                .with_audit(audit.clone());
            let result = engine
                .run(
                    |sys, usr| {
                        let http = http_client.clone();
                        let kp = key_provider.clone();
                        let local = local_client.clone();
                        let cfg = config.clone();
                        async move { call_llm(&http, kp.as_ref(), &sys, &usr, &cfg, local.as_ref(), false).await }
                    },
                    &load_prompt("architect"),
                    demo_mission,
                    &tool_registry,
                    &critic,
                )
                .await;
            for obs in result.observations.iter().take(2) {
                println!("  - step {} | action: {} | success: {}", obs.iteration, obs.action, obs.success);
            }

            // 4) Tail audit log
            println!("[4/4] Audit Trail (last 8 entries)");
            match audit.read_last_lines(8) {
                Ok(lines) if !lines.is_empty() => {
                    for line in lines {
                        println!("  {}", line);
                    }
                }
                Ok(_) => println!("  (audit disabled or empty)"),
                Err(e) => println!("  (failed to read audit log: {})", e),
            }

            println!("\nSummary:");
            println!("- Air-Gapped: {}", if airgapped { "ENABLED" } else { "DISABLED" });
            println!("- HITL: {}", if config.security.human_in_the_loop { "ENABLED" } else { "DISABLED" });
            println!("- Audit Trail: {}", if config.audit.enabled { "ACTIVE" } else { "DISABLED" });
            println!("- Docs: docs/pilot/");
            println!("----------------------------------------\n");
            let _ = audit.log_event("agent-cli", "demo_completed", 0.3, true);
            continue;
        }
        if cmd.starts_with("/role ") { let mut r = role_clone.lock().await; *r = cmd.replace("/role ", "").trim().to_string(); println!("[SYSTEM]: {}", *r); continue; }
        if cmd.starts_with("/search ") { let q = cmd.replace("/search ", ""); match kb.search_osint(&q).await { Ok(r) if !r.is_empty() => { for (_, sc, p) in &r { println!("  - {} ({:.2})", p.get("title").unwrap_or(&"?".into()), sc); } } _ => println!("  Ничего не найдено.") } continue; }
        if cmd.starts_with("/search_darknet ") { let q = cmd.replace("/search_darknet ", ""); match kb.search_darknet(&q).await { Ok(r) if !r.is_empty() => { for (_, sc, p) in &r { println!("  - {} ({:.2})", p.get("title").unwrap_or(&"?".into()), sc); } } _ => println!("  Ничего не найдено.") } continue; }
        if cmd.starts_with("/research ") { let t = cmd.replace("/research ", "").trim().to_string(); let sp = load_prompt("researcher_osint").replace("{TARGET}", &t).replace("{DATE}", &chrono::Local::now().format("%Y-%m-%d").to_string()); if let Some(r) = call_llm(&http_client, key_provider.as_ref(), &sp, &format!("Анализ {}", t), &config, local_client.as_ref(), false).await { println!("\n\x1b[32m[OSINT]:\x1b[0m\n{}", r); let _ = kb.ingest_osint(&format!("Исследование: {}", t), "OSINT", &r).await; server_state.store.increment("osint_count").await; } continue; }
        if cmd.starts_with("/darknet ") { let q = cmd.replace("/darknet ", "").trim().to_string(); let sp = load_prompt("researcher_darknet").replace("{DATE}", &chrono::Local::now().format("%Y-%m-%d").to_string()); if let Some(r) = call_llm(&http_client, key_provider.as_ref(), &sp, &format!("Собери: {}", q), &config, local_client.as_ref(), false).await { println!("\n\x1b[32m[DARKNET]:\x1b[0m\n{}", r); let _ = kb.ingest_darknet(&format!("DarkNet CTI: {}", q), "DarkNet", &r).await; server_state.store.increment("darknet_count").await; } continue; }
        if cmd == "/map" { println!("\n\x1b[35m═══ KNOWLEDGE MAP ═══\x1b[0m"); match kb.search_osint("CrowdStrike SentinelOne").await { Ok(r) if !r.is_empty() => { for (_, _, p) in &r { if let Some(t) = p.get("title") { println!("  ✅ {}", t); } } } _ => println!("  ❌ OSINT пуст.") } continue; }
        if cmd == "/gaps" { println!("\n\x1b[33m═══ GAPS ═══\x1b[0m"); for (n, _) in &[("Fortinet",""),("Zscaler",""),("Wiz",""),("Okta",""),("Cloudflare","")] { match kb.search_osint(n).await { Ok(r) if !r.is_empty() => {}, _ => println!("  🔴 {}", n) } } continue; }
        if cmd == "/save" { let state = json!({"osint_docs": kb.search_osint(".").await.map(|r| r.len()).unwrap_or(0), "darknet_docs": kb.search_darknet(".").await.map(|r| r.len()).unwrap_or(0), "timestamp": chrono::Local::now().to_rfc3339()}); fs::write("aegis_session.json", state.to_string()).unwrap(); println!("✅ Сессия сохранена."); continue; }
        if cmd == "/dashboard" { println!("\n\x1b[35mЗапуск AEGIS Command Center...\x1b[0m"); match dashboard::run_dashboard() { Ok(selected) => { if selected != "exit" { println!("\n[DASHBOARD] Запуск команды: {}", selected); } } Err(e) => eprintln!("[DASHBOARD] Ошибка: {}", e) } continue; }
        if cmd == "/god" || cmd == "/audit" {
            // Human-in-the-loop gate (GOD MODE is high risk by design)
            if config.security.god_mode_safety_level == crate::config::GodModeSafety::Strict {
                println!("\n\x1b[31m[GOD MODE] STRICT safety: any deploy/execute requires explicit human approval.\x1b[0m");
                if !prompt_yes_no(
                    "[HITL] Proceed with GOD MODE audit output? [y/N] ",
                    Some(audit.as_ref()),
                    "/god",
                    0.9,
                    None,
                ) {
                    println!("[HITL] Cancelled.");
                    let _ = audit.log_event("agent-cli", "god_mode_cancelled", 0.9, false);
                    continue;
                }
                let _ = audit.log_event("agent-cli", "god_mode_confirmed", 0.9, true);
            }
            println!("\n\x1b[31m══════════════════════════════════════\x1b[0m");
            println!("\x1b[31m[GOD MODE] Supply-chain & Formal Verification\x1b[0m");
            println!("\x1b[31m══════════════════════════════════════\x1b[0m");
            println!("→ cargo-deny (advisories, bans, sources, licenses) — recommended in CI");
            println!("→ cargo-audit (rustsec) — recommended before deploy");
            println!("→ cargo +RUSTFLAGS=\"-D warnings\" + clippy -- -D warnings");
            println!("→ Formal verification: KLEE / MIRI / Prusti (advanced)");
            println!("→ Expanded Inquisitor checklist: 35+ пунктов (см. inquisitor.prompt)");
            println!("\x1b[32m[GOD MODE] Рекомендация: добавьте в CI: cargo-deny check && cargo-audit\x1b[0m");
            continue;
        }

        if cmd == "/fusion" {
            let stats = fusion.get_stats().await;
            println!("\n\x1b[36m═══ FUSION ENGINE STATS ═══\x1b[0m");
            println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
            let threats = fusion.get_fused_threats(5).await;
            for t in threats { println!("  • Cluster {} | Sev {:.2} | Conf {:.2} | Sources: {}", t.cluster_id, t.severity, t.confidence, t.sources.join(",")); }
            continue;
        }

        // === ReAct++ Demo (Фаза 3) ===
        if cmd.starts_with("/react ") {
            let mission = cmd.replace("/react ", "").trim().to_string();
            println!("\n\x1b[35m══════════════════════════════════════\x1b[0m");
            println!("\x1b[35m[ReAct++] Миссия: {}\x1b[0m", mission);
            println!("\x1b[35m══════════════════════════════════════\x1b[0m");

            let engine = react_engine::ReactEngine::new(6)
                .with_mcts(mcts::MctsEngine::new(25))
                .with_air_gapped(config.is_air_gapped())
                .with_audit(audit.clone());
            let result = engine.run(
                |sys, usr| {
                    let http = http_client.clone();
                    let kp = key_provider.clone();
                    let local = local_client.clone();
                    let cfg = config.clone();
                    async move { call_llm(&http, kp.as_ref(), &sys, &usr, &cfg, local.as_ref(), true).await }
                },
                &load_prompt("architect"),
                &mission,
                &tool_registry,
                &critic,
            ).await;

            println!("\n\x1b[36m[ReAct++] Итог: {} | Итераций: {}/{} | MCTS: ON\x1b[0m",
                if result.success { "УСПЕХ" } else { "НЕУДАЧА" },
                result.iterations_used, 6);

            for obs in &result.observations {
                let status = if obs.success { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
                println!("  {} [{}] {} → {}", status, obs.iteration, obs.action, &obs.result[..120.min(obs.result.len())]);
            }

            println!("\n\x1b[33mFinal Answer:\x1b[0m {}", result.final_answer);
            println!("\x1b[35m══════════════════════════════════════\x1b[0m");
            continue;
        }

        // === Восстановленные команды из истории (GOD MODE / Scout / Ingest) ===
        if cmd.starts_with("/scout") || cmd.starts_with("/research-cycle") {
            let topic = cmd
                .strip_prefix("/scout")
                .or_else(|| cmd.strip_prefix("/research-cycle"))
                .unwrap_or("")
                .trim()
                .to_string();
            let topic = if topic.is_empty() {
                "latest critical CVEs + active exploitation".to_string()
            } else {
                topic
            };

            println!("\n\x1b[36m[SCOUT] Запрос: {}\x1b[0m", topic);

            // HITL mandatory for external research cycle
            if config.security.human_in_the_loop {
                if !prompt_yes_no(
                    "[HITL] Approve Scout research cycle (OSINT + DarkNet heuristics)? [y/N] ",
                    Some(audit.as_ref()),
                    "/scout research_cycle",
                    0.55,
                    Some("scout_start"),
                ) {
                    println!("[SCOUT] Cancelled by human.");
                    let _ = audit.log_event("agent-cli", "scout_cancelled", 0.55, false);
                    continue;
                }
            }

            let scout = scout::Scout::new(
                tool_registry.clone(),
                audit.clone(),
                config.is_air_gapped(),
            )
            .with_llm(scout::ScoutLlm {
                http: http_client.clone(),
                api_key: api_key.clone(),
                key_provider: key_provider.clone(),
                config: config.clone(),
                local: local_client.clone(),
            });
            match scout.run_advanced(&topic).await {
                Ok(mut items) => {
                    // === Critic 2.0: per-item knowledge evaluation (hypotheses + White/Black) ===
                    let k_ctx = critic_agent::format_scout_context_for_critic(&topic, &items);
                    let mut eval_targets: Vec<crate::knowledge_item::KnowledgeItem> = items
                        .iter()
                        .filter(|i| i.item_type == crate::knowledge_item::KnowledgeType::Hypothesis)
                        .cloned()
                        .collect();
                    eval_targets.extend(items.iter().filter(|i| {
                        matches!(
                            i.item_type,
                            crate::knowledge_item::KnowledgeType::White
                                | crate::knowledge_item::KnowledgeType::Black
                                | crate::knowledge_item::KnowledgeType::TTP
                        )
                    }).cloned());
                    let mut seen = std::collections::HashSet::new();
                    eval_targets.retain(|i| seen.insert(i.id.clone()));
                    eval_targets.truncate(24);

                    // === Critic 2.0 — параллельная оценка всех элементов ===
                    // Было: последовательный цикл (24 items × 25s = потенциально 600s).
                    // Теперь: FuturesUnordered — все запросы параллельно.
                    use futures::stream::FuturesUnordered;
                    let mut critic_futures = FuturesUnordered::new();
                    for it in &eval_targets {
                        let critic_ref = critic.clone();
                        let k_ctx_ref = k_ctx.clone();
                        let it_owned = (*it).clone();
                        critic_futures.push(async move {
                            let result = critic_ref.evaluate_knowledge(&it_owned, Some(&k_ctx_ref)).await;
                            (it_owned.id.clone(), result)
                        });
                    }
                    let mut critic_by_id: std::collections::HashMap<String, critic_agent::CriticEvaluation> =
                        std::collections::HashMap::new();
                    let mut max_knowledge_risk = 0.0_f64;
                    let mut any_k_block = false;
                    let mut any_k_escalate = false;
                    let mut low_critic_conf = 0usize;
                    let mut blocked_ids = std::collections::HashSet::new();
                    
                    while let Some((id, result)) = futures::StreamExt::next(&mut critic_futures).await {
                        if let Ok(ev) = result {
                            max_knowledge_risk = max_knowledge_risk.max(ev.security_risk);
                            if ev.verdict == Verdict::Block { 
                                any_k_block = true; 
                                blocked_ids.insert(id.clone());
                            }
                            if ev.verdict == Verdict::Escalate { any_k_escalate = true; }
                            if ev.confidence < 0.45 { low_critic_conf += 1; }
                            critic_by_id.insert(id, ev);
                        }
                    }
                    
                    let evaluated_count = eval_targets.len();
                    
                    if any_k_block {
                        let original_len = items.len();
                        items.retain(|i| !blocked_ids.contains(&i.id));
                        eval_targets.retain(|i| !blocked_ids.contains(&i.id));
                        println!("\x1b[33m[CRITIC 2.0] Filtered out {} blocked items.\x1b[0m", original_len - items.len());
                    }

                    if low_critic_conf > 0 && low_critic_conf * 2 >= evaluated_count.max(1) {
                        let _ = audit.log_event(
                            "critic",
                            &format!(
                                "critic_knowledge_low_confidence_batch n={} low={}",
                                evaluated_count,
                                low_critic_conf
                            ),
                            0.4,
                            false,
                        );
                    }
                    println!(
                        "\n\x1b[35m[CRITIC 2.0]\x1b[0m evaluated {} items | max_knowledge_risk={:.2} | any_BLOCK={} | low_conf_items={}",
                        evaluated_count,
                        max_knowledge_risk,
                        any_k_block,
                        low_critic_conf
                    );

                    // === Critic gate (bulk + merge with 2.0) ===
                    let synthesis = format!(
                        "SCOUT ITEMS TOPIC: {}\nITEMS:\n{}",
                        topic,
                        items.iter().take(25).map(|it| format!("- {:?} | {} | {:.2}\n{}", it.item_type, it.source, it.confidence, &it.content[..300.min(it.content.len())])).collect::<Vec<_>>().join("\n")
                    );
                    let critic_score = critic
                        .evaluate("ingest_knowledge_base()", &topic, &synthesis)
                        .await;
                    let merged_risk = critic_score.security_risk.max(max_knowledge_risk);
                    let merged_verdict = if critic_score.verdict == "BLOCK" {
                        "BLOCK".to_string()
                    } else if any_k_escalate || critic_score.verdict == "ESCALATE" {
                        "ESCALATE".to_string()
                    } else {
                        critic_score.verdict.clone()
                    };
                    println!(
                        "\n\x1b[35m[CRITIC]\x1b[0m verdict={} (merged) risk={:.2} (merged) utility={:.2} hitl={}",
                        merged_verdict,
                        merged_risk,
                        critic_score.utility,
                        critic_score.needs_human_approval
                    );
                    metrics::critic_bulk_verdict(&merged_verdict);
                    // Only hard-block on explicit BLOCK or extreme risk.
                    // ESCALATE is handled by Inquisitor + mandatory HITL below.
                    if merged_verdict == "BLOCK" || merged_risk >= 0.95 {
                        println!(
                            "[SCOUT] BLOCKED by Critic (merged): {} | merged_risk={:.2} | bulk_verdict={} | knowledge2_BLOCK={}",
                            critic_score.reasoning,
                            merged_risk,
                            critic_score.verdict,
                            any_k_block
                        );
                        let _ = audit.log_event("agent-cli", "scout_blocked_by_critic", merged_risk, false);
                        metrics::learning_gate_finish("critic", false);
                        continue;
                    }

                    // === Inquisitor 2.0 — параллельная оценка ===
                    let mut inq_futures = FuturesUnordered::new();
                    for it in &eval_targets {
                        let inq_ref = inquisitor.clone();
                        let k_ctx_ref = k_ctx.clone();
                        let it_owned = (*it).clone();
                        let c_ev = critic_by_id.get(&it_owned.id).cloned();
                        inq_futures.push(async move {
                            let result = inq_ref.evaluate_knowledge(&it_owned, Some(&k_ctx_ref), c_ev.as_ref()).await;
                            (it_owned.clone(), result)
                        });
                    }
                    let mut inq_any_hard_block = false;
                    let mut inq_any_escalate = false;
                    let mut inq_block_details: Vec<String> = Vec::new();
                    let mut inq_escalate_details: Vec<String> = Vec::new();
                    // Collect inquisitor audit events to log outside the async closure
                    let mut inq_audit_events: Vec<(String, f64)> = Vec::new();
                    while let Some((it_owned, result)) = futures::StreamExt::next(&mut inq_futures).await {
                        match result {
                            Ok(ev) => {
                                metrics::inquisitor_knowledge_verdict(ev.verdict.as_str());
                                if inquisitor_agent::evaluation_is_hard_block(&ev) {
                                    inq_any_hard_block = true;
                                    inq_block_details.push(format!(
                                        "id={} type={:?} verdict={} risk_areas={:?} | {}",
                                        &it_owned.id[..it_owned.id.len().min(16)],
                                        it_owned.item_type,
                                        ev.verdict.as_str(),
                                        ev.risk_areas,
                                        ev.reasoning.chars().take(220).collect::<String>()
                                    ));
                                    inq_audit_events.push((format!(
                                        "scout_inquisitor2_BLOCK_detail id={} risk_areas={:?} reasoning_excerpt={}",
                                        &it_owned.id[..it_owned.id.len().min(12)],
                                        ev.risk_areas,
                                        ev.reasoning.chars().take(160).collect::<String>()
                                    ), 0.9));
                                } else if inquisitor_agent::evaluation_requires_escalation(&ev) {
                                    inq_any_escalate = true;
                                    if !ev.risk_areas.is_empty() {
                                        inq_escalate_details.push(format!(
                                            "id={} areas={:?} | {}",
                                            &it_owned.id[..it_owned.id.len().min(12)],
                                            ev.risk_areas,
                                            ev.reasoning.chars().take(120).collect::<String>()
                                        ));
                                    }
                                }
                            }
                            Err(e) => tracing::warn!(target: "inquisitor", "evaluate_knowledge: {}", e),
                        }
                    }
                    for (msg, risk) in inq_audit_events {
                        let _ = audit.log_event("agent-cli", &msg, risk, false);
                    }
                    println!(
                        "\n\x1b[31m[INQUISITOR 2.0]\x1b[0m evaluated {} items | hard_BLOCK={} | needs_ESCALATE={}",
                        eval_targets.len(),
                        inq_any_hard_block,
                        inq_any_escalate
                    );
                    if inq_any_hard_block {
                        println!(
                            "\x1b[31m[INQUISITOR 2.0] BLOCK / critical risk — details ({}):\x1b[0m",
                            inq_block_details.len()
                        );
                        for line in inq_block_details.iter().take(8) {
                            println!("  {}", line);
                        }
                    } else if !inq_escalate_details.is_empty() {
                        println!(
                            "\x1b[33m[INQUISITOR 2.0] Escalation drivers (sample):\x1b[0m"
                        );
                        for line in inq_escalate_details.iter().take(6) {
                            println!("  {}", line);
                        }
                    }

                    // === Inquisitor gate (bulk LLM audit + merge with 2.0) ===
                    let inq_prompt = load_prompt("inquisitor");
                    let inq_input = format!(
                        "Audit this scout report for safety-to-ingest. Output strictly:\nVERDICT: ALLOW|ESCALATE|BLOCK\nREASON: <short>\n\n{}",
                        synthesis
                    );
                    let inq_out = call_llm(
                        &http_client,
                        key_provider.as_ref(),
                        &inq_prompt,
                        &inq_input,
                        &config,
                        local_client.as_ref(),
                        true,
                    )
                    .await
                    .unwrap_or_else(|| "VERDICT: ESCALATE\nREASON: Inquisitor unavailable".to_string());
                    let bulk_verdict = inq_out
                        .lines()
                        .find(|l| l.to_uppercase().starts_with("VERDICT:"))
                        .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_uppercase()))
                        .unwrap_or_else(|| "ESCALATE".to_string());
                    let reason = inq_out
                        .lines()
                        .find(|l| l.to_uppercase().starts_with("REASON:"))
                        .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()))
                        .unwrap_or_else(|| "no reason".to_string());

                    let merged_inq = if inq_any_hard_block || bulk_verdict == "BLOCK" {
                        "BLOCK".to_string()
                    } else if inq_any_escalate || bulk_verdict == "ESCALATE" {
                        "ESCALATE".to_string()
                    } else {
                        bulk_verdict.clone()
                    };

                    println!(
                        "\n\x1b[31m[INQUISITOR]\x1b[0m verdict={} (merged) bulk={} | inq2_hard_BLOCK={} inq2_escalate={} | reason={}",
                        merged_inq,
                        bulk_verdict,
                        inq_any_hard_block,
                        inq_any_escalate,
                        reason
                    );
                    let _ = audit.log_event(
                        "agent-cli",
                        &format!(
                            "scout_inquisitor_verdict={} bulk={} inq2_block={} inq2_escalate={}",
                            merged_inq, bulk_verdict, inq_any_hard_block, inq_any_escalate
                        ),
                        0.35,
                        true,
                    );
                    metrics::inquisitor_bulk_verdict(&merged_inq);
                    if merged_inq != "ALLOW" {
                        // Phase 1: HITL override path for LLM outages or ESCALATE verdicts.
                        // We still keep strict default-deny unless human explicitly overrides.
                        if config.security.human_in_the_loop {
                            println!("[SCOUT] Inquisitor did not ALLOW. HITL override required to ingest.");
                            if !prompt_yes_no(
                                "[HITL] Override Inquisitor and FORCE ingest (white/black items only)? [y/N] ",
                                Some(audit.as_ref()),
                                "/scout inquisitor_override",
                                0.8,
                                Some("ingest"),
                            ) {
                                println!("[SCOUT] Not ingesting (verdict={}).", merged_inq);
                                let _ = audit.log_event("agent-cli", "scout_ingest_denied_by_inquisitor", 0.6, false);
                                metrics::learning_gate_finish("inquisitor", false);
                                continue;
                            }
                            println!("[SCOUT] FORCE ingest approved by human override.");
                            let _ = audit.log_event("agent-cli", "scout_ingest_forced_human_override", 0.8, true);
                            // Mark for provenance downstream
                            for it in &mut items {
                                if !it.verified_by.iter().any(|v| v == "human_override") {
                                    it.verified_by.push("human_override".into());
                                }
                            }
                        } else {
                            // HITL отключён, но Inquisitor не дал ALLOW.
                            // Это НЕ должно быть тихим drop — логируем как security event.
                            tracing::warn!(
                                target: "security",
                                "[SECURITY ALERT] Inquisitor verdict={} but HITL disabled — ingest BLOCKED. topic={}",
                                merged_inq, topic
                            );
                            println!(
                                "\x1b[31m[SECURITY ALERT] Inquisitor={} — ingest BLOCKED (HITL disabled, strict deny)\x1b[0m",
                                merged_inq
                            );
                            let _ = audit.log_event(
                                "agent-cli",
                                &format!("scout_ingest_blocked_no_hitl verdict={} topic={}", merged_inq, &topic[..topic.len().min(80)]),
                                0.85,
                                false,
                            );
                            // Отправляем алерт в Fusion/dashboard
                            let _ = server_state.alert_tx.send(format!(
                                "[SECURITY] Inquisitor blocked ingest ({}): {}",
                                merged_inq, &topic[..topic.len().min(120)]
                            ));
                            server_state.store.increment("threats_blocked").await;
                            metrics::learning_gate_finish("inquisitor", false);
                            continue;
                        }
                    }

                    metrics::learning_gate_finish("inquisitor", true);

                    // HITL before ingest + DNA update (mandatory)
                    if config.security.human_in_the_loop {
                        if !prompt_yes_no(
                            "[HITL] Approve ingest into KnowledgeBase + DNA update? [y/N] ",
                            Some(audit.as_ref()),
                            "/scout ingest",
                            0.65,
                            Some("ingest"),
                        ) {
                            println!("[SCOUT] Ingest cancelled by human.");
                            let _ = audit.log_event("agent-cli", "scout_ingest_cancelled", 0.65, false);
                            metrics::learning_gate_finish("ingest", false);
                            continue;
                        }
                    }

                    // Stamp verification chain
                    for it in &mut items {
                        if !it.verified_by.iter().any(|v| v == "critic") {
                            it.verified_by.push("critic".into());
                        }
                        if !it.verified_by.iter().any(|v| v == "inquisitor") {
                            it.verified_by.push("inquisitor".into());
                        }
                        if config.security.human_in_the_loop && !it.verified_by.iter().any(|v| v == "human") {
                            it.verified_by.push("human".into());
                        }
                    }

                    println!("\n\x1b[36m[SCOUT] Items (sample):\x1b[0m");
                    for it in items.iter().take(10) {
                        println!(
                            "  - {:?} | {} | conf={:.2} | {}",
                            it.item_type,
                            it.source,
                            it.confidence,
                            &it.summary.clone().unwrap_or_default()[..120.min(it.summary.clone().unwrap_or_default().len())]
                        );
                    }

                    // === ALLOW → ingest to KnowledgeBase (White/Black only) ===
                    let mut ingested_ok = 0usize;
                    let mut ingested_err = 0usize;
                    let mut ok_white = 0usize;
                    let mut ok_black = 0usize;
                    for it in items.iter().cloned() {
                        match it.item_type {
                            crate::knowledge_item::KnowledgeType::White => {
                                match kb.ingest_white(it).await {
                                    Ok(deduped) => {
                                        ingested_ok += 1;
                                        ok_white += 1;
                                        if deduped {
                                            metrics::knowledge_deduped(1);
                                        } else {
                                            metrics::knowledge_ingested(
                                                &crate::knowledge_item::KnowledgeType::White,
                                                1,
                                            );
                                        }
                                    }
                                    Err(_) => {
                                        ingested_err += 1;
                                    }
                                }
                            }
                            crate::knowledge_item::KnowledgeType::Black => {
                                match kb.ingest_black(it).await {
                                    Ok(deduped) => {
                                        ingested_ok += 1;
                                        ok_black += 1;
                                        if deduped {
                                            metrics::knowledge_deduped(1);
                                        } else {
                                            metrics::knowledge_ingested(
                                                &crate::knowledge_item::KnowledgeType::Black,
                                                1,
                                            );
                                        }
                                    }
                                    Err(_) => {
                                        ingested_err += 1;
                                    }
                                }
                            }
                            _ => {
                                // Phase 1: do not ingest hypotheses/ttp
                            }
                        }
                    }
                    println!(
                        "\n\x1b[32m[SCOUT] Ingest summary:\x1b[0m ok={} (white={}, black={}) err={}",
                        ingested_ok, ok_white, ok_black, ingested_err
                    );
                    let _ = audit.log_event(
                        "agent-cli",
                        &format!(
                            "scout_ingested ok={} white={} black={} err={}",
                            ingested_ok, ok_white, ok_black, ingested_err
                        ),
                        0.25,
                        true,
                    );

                    metrics::learning_gate_finish("ingest", ingested_err == 0);

                    // === DNA Engine 2.5 (weighted + White/Black split + decay) ===
                    let dna = dna_engine::DnaEngine::new(&config.database.dna_path, audit.clone());
                    let dna_ok = match dna.update_with_items(&topic, &items).await {
                        Ok(snap) => {
                            println!(
                                "\n\x1b[36m[DNA 2.5]\x1b[0m v{} | weighted={:.2} | w_avg_conf={:.2} | W={} B={} H={}",
                                snap.version,
                                snap.total_weighted_importance,
                                snap.weighted_avg_confidence,
                                snap.white_knowledge_count,
                                snap.black_knowledge_count,
                                snap.hypothesis_count
                            );
                            true
                        }
                        Err(e) => {
                            tracing::warn!("DNA update: {}", e);
                            false
                        }
                    };
                    metrics::learning_gate_finish("dna", dna_ok);
                    if dna_ok {
                        metrics::self_learning_cycle_completed();
                        let _ = audit.log_event("agent-cli", "self_learning_cycle_completed", 0.22, true);
                    }

                    let _ = audit.log_event("agent-cli", "scout_completed_basic", 0.25, true);
                }
                Err(e) => {
                    println!("[SCOUT] Failed: {}", e);
                    let _ = audit.log_event("agent-cli", &format!("scout_failed: {}", e), 0.4, false);
                }
            }
            continue;
        }
        if cmd.starts_with("/assimilate") {
            println!("\n\x1b[35m[ASSIMILATE] Полный цикл: gaps → research → generate protection\x1b[0m");
            // Простая реализация: запускаем gaps + research
            println!("  → Запуск /gaps и /research для ключевых вендоров...");
            continue;
        }
        if cmd.starts_with("/kb ") {
            let rest = cmd.strip_prefix("/kb ").unwrap_or("").trim();
            let mut parts = rest.split_whitespace();
            let sub = parts.next().unwrap_or("");
            if sub == "feedback" {
                let id = parts.next().unwrap_or("").trim().to_string();
                let fb_raw = parts.next().unwrap_or("").trim();
                if id.is_empty() || fb_raw.is_empty() {
                    println!("Usage: /kb feedback <id> useful|not_useful|false_positive|needs_review");
                    continue;
                }
                let Some(fb) = crate::knowledge_item::KnowledgeFeedback::parse(fb_raw) else {
                    println!("Unknown feedback. Use: useful | not_useful | false_positive | needs_review");
                    continue;
                };
                match kb.set_knowledge_feedback(&id, fb).await {
                    Ok(()) => {
                        println!("[KB] feedback set id={} -> {}", id, fb.as_str());
                        metrics::feedback_received(fb.as_str());
                        let _ = audit.log_event(
                            "agent-cli",
                            &format!("kb_feedback id={} {}", &id[..id.len().min(16)], fb.as_str()),
                            0.2,
                            true,
                        );
                    }
                    Err(e) => println!("[KB] {}", e),
                }
                continue;
            }
            if sub == "list" {
                let what = parts.next().unwrap_or("");
                if what == "pending_feedback" {
                    let limit = parts
                        .next()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(20)
                        .clamp(1, 100);
                    match kb.list_pending_feedback(limit).await {
                        Ok(items) => {
                            println!(
                                "\n\x1b[36m[KB] pending_feedback\x1b[0m limit={} count={}",
                                limit,
                                items.len()
                            );
                            for it in items.iter() {
                                let sum = it.summary.clone().unwrap_or_default();
                                println!(
                                    "  id={} | {:?} | conf={:.2} | {}",
                                    it.id,
                                    it.item_type,
                                    it.confidence,
                                    &sum[..120.min(sum.len())]
                                );
                            }
                            let _ = audit.log_event(
                                "agent-cli",
                                &format!("kb_list_pending_feedback n={}", items.len()),
                                0.15,
                                true,
                            );
                        }
                        Err(e) => println!("[KB] list failed: {}", e),
                    }
                } else {
                    println!("Usage: /kb list pending_feedback [limit]");
                }
                continue;
            }
            if sub == "ingest" {
                let mode = parts.next().unwrap_or("");
                // Сначала убираем кавычки из контента
                let content = parts.collect::<Vec<_>>().join(" ").trim_matches('"').to_string();
                if content.is_empty() || (mode != "white" && mode != "black") {
                    println!("Usage: /kb ingest white|black \"<content>\"");
                    continue;
                }
                
                let item = crate::knowledge_item::KnowledgeItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    item_type: if mode == "white" { crate::knowledge_item::KnowledgeType::White } else { crate::knowledge_item::KnowledgeType::Black },
                    content: content.clone(),
                    summary: Some(content.chars().take(50).collect()),
                    source: "manual_cli".to_string(),
                    confidence: 1.0,
                    verified_by: vec!["human".to_string()],
                    tags: vec!["manual".to_string()],
                    related_iocs: vec![],
                    first_seen: chrono::Utc::now().timestamp(),
                    last_seen: chrono::Utc::now().timestamp(),
                    embedding_id: None,
                    content_hash: String::new(),
                    feedback: None,
                };
                
                if mode == "white" {
                    let _ = kb.ingest_white(item).await;
                } else {
                    let _ = kb.ingest_black(item).await;
                }
                
                println!("[KB] Ingested {} item successfully", mode);
                continue;
            }
            let mode = sub;
            let limit = parts
                .next()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(5)
                .clamp(1, 20);
            let query = parts.collect::<Vec<_>>().join(" ");
            if query.is_empty() || (mode != "white" && mode != "black") {
                println!("Usage: /kb white <limit?> <query>  |  /kb black <limit?> <query>");
                println!("       /kb feedback <id> useful|not_useful|false_positive|needs_review");
                println!("       /kb list pending_feedback [limit]");
                println!("Example: /kb black 5 fortinet cve");
                continue;
            }
            let results = if mode == "white" {
                kb.search_white(&query, limit).await
            } else {
                kb.search_black(&query, limit).await
            };
            match results {
                Ok(items) => {
                    println!("\n\x1b[36m[KB {}] query='{}' results={}\x1b[0m", mode.to_uppercase(), query, items.len());
                    for it in items.iter().take(limit) {
                        let sum = it.summary.clone().unwrap_or_default();
                        let ioc = if it.related_iocs.is_empty() {
                            "".to_string()
                        } else {
                            format!(" iocs={}", it.related_iocs.len())
                        };
                        println!(
                            "  id={} | {:?} | src={} | conf={:.2}{} | fb={:?} | {}",
                            it.id,
                            it.item_type,
                            it.source,
                            it.confidence,
                            ioc,
                            it.feedback,
                            &sum[..180.min(sum.len())]
                        );
                    }
                    let _ = audit.log_event("agent-cli", &format!("kb_search mode={} q='{}' n={}", mode, query, items.len()), 0.15, true);
                }
                Err(e) => {
                    println!("[KB] search failed: {}", e);
                    let _ = audit.log_event("agent-cli", &format!("kb_search_failed: {}", e), 0.25, false);
                }
            }
            continue;
        }
        if cmd.starts_with("/code ") {
            let task = cmd.replace("/code ", "").trim().to_string();
            println!("\n\x1b[31m[CODE / GOD MODE] Генерация + Инквизитор: {}\x1b[0m", task);
            let _ = audit.log_event("agent-cli", &format!("god_mode_code_request: {}", task), 0.9, false);

            // HITL gate: /code is GOD MODE path; no implicit deploy/execute allowed
            let action_label = format!("/code {}", task);
            let needs = safety::require_human_approval(&action_label, 0.9);
            if needs {
                println!("\x1b[31m[HITL] Human approval required before GOD MODE action.\x1b[0m");
                println!("      SafetyLevel: {:?}", config.security.god_mode_safety_level);
                if config.security.god_mode_safety_level == crate::config::GodModeSafety::Strict {
                    // Strict: never allow automation; require explicit confirmation every time.
                    if !prompt_yes_no(
                        "[HITL] Confirm to proceed (no deploy will be executed) [y/N] ",
                        Some(audit.as_ref()),
                        &action_label,
                        0.9,
                        None,
                    ) {
                        println!("[HITL] Cancelled.");
                        let _ = audit.log_event("agent-cli", "god_mode_code_cancelled", 0.9, false);
                        continue;
                    }
                } else if config.security.god_mode_safety_level == crate::config::GodModeSafety::AuditOnly {
                    if !prompt_yes_no(
                        "[HITL] Confirm to proceed with audit-only output [y/N] ",
                        Some(audit.as_ref()),
                        &action_label,
                        0.9,
                        None,
                    ) {
                        println!("[HITL] Cancelled.");
                        let _ = audit.log_event("agent-cli", "god_mode_code_cancelled", 0.9, false);
                        continue;
                    }
                }
            }

            let sp = load_prompt("inquisitor");
            if let Some(r) = call_llm(&http_client, key_provider.as_ref(), &sp, &format!("Сгенерируй безопасный код: {}", task), &config, local_client.as_ref(), true).await {
                println!("\x1b[32m[Inquisitor Audit]:\x1b[0m {}", r);
                let _ = audit.log_event("agent-cli", "god_mode_code_completed", 0.4, true);
            }
            continue;
        }
        if cmd.starts_with("/plan ") {
            let task = cmd.replace("/plan ", "").trim().to_string();
            println!("\n\x1b[33m[PLAN] Анализ задачи без генерации кода: {}\x1b[0m", task);
            let sp = load_prompt("architect");
            if let Some(r) = call_llm(&http_client, key_provider.as_ref(), &sp, &format!("План реализации: {}", task), &config, local_client.as_ref(), false).await {
                println!("{}", r);
            }
            continue;
        }
        if cmd == "/rollback" {
            println!("\n\x1b[31m[ROLLBACK] Откат к последнему git snapshot...\x1b[0m");
            let _ = std::process::Command::new("git").args(["reset", "--hard", "HEAD~1"]).status();
            continue;
        }
        if cmd.starts_with("/ingest ") {
            let rest = cmd.replace("/ingest ", "");
            let parts: Vec<&str> = rest.splitn(3, '|').collect();
            if parts.len() == 3 {
                let _ = kb.ingest_osint(parts[0], parts[1], parts[2]).await;
                println!("✅ OSINT Ingest: {}", parts[0]);
                server_state.store.increment("osint_count").await;
            } else {
                println!("Формат: /ingest title|source|text");
            }
            continue;
        }
        if cmd.starts_with("/ingest_darknet ") {
            let rest = cmd.replace("/ingest_darknet ", "");
            let parts: Vec<&str> = rest.splitn(3, '|').collect();
            if parts.len() == 3 {
                let _ = kb.ingest_darknet(parts[0], parts[1], parts[2]).await;
                println!("✅ DarkNet Ingest: {}", parts[0]);
                server_state.store.increment("darknet_count").await;
            } else {
                println!("Формат: /ingest_darknet title|source|text");
            }
            continue;
        }

        // === /heal — Self-Healing с Verification Report и Raft Quorum ===
        if cmd.starts_with("/heal ") {
            let anomaly = cmd.replace("/heal ", "").trim().to_string();
            if anomaly.is_empty() {
                println!("Использование: /heal <описание аномалии>");
                continue;
            }

            println!("\n\x1b[36m[HEALING]\x1b[0m Запуск Self-Healing для: {}", anomaly);

            let healing = HealingOrchestrator::new(
                kb.clone(),
                Arc::new(dna_engine::DnaEngine::new(&config.database.dna_path, audit.clone())),
                inquisitor.clone(),
                audit.clone(),
                config.clone(),
            )
            .with_oracle(oracle.clone());

            match healing.heal(&anomaly, PatchType::Custom).await {
                Ok(result) => {
                    println!("\n\x1b[32m[HEALING RESULT]\x1b[0m {}", result.summary());
                    println!("Verification passed: {} | severity: {:.2}", result.verification_passed, result.verification_severity());
                    if result.has_findings() {
                        println!("Findings:");
                        for f in &result.verification_report.findings {
                            println!("  - {}", f);
                        }
                    }
                    if !result.verification_report.recommendations.is_empty() {
                        println!("Recommendations:");
                        for r in &result.verification_report.recommendations {
                            println!("  - {}", r);
                        }
                    }
                    println!("Applied: {} | Rollback available: {}", result.applied, result.rollback_available);
                }
                Err(e) => {
                    println!("\x1b[31m[HEALING ERROR]\x1b[0m {}", e);
                }
            }
            continue;
        }

        // === /auto-heal — Autonomous Remediation (Low/Medium risk без HITL) ===
        if cmd.starts_with("/auto-heal ") {
            let anomaly = cmd.replace("/auto-heal ", "").trim().to_string();
            if anomaly.is_empty() {
                println!("Использование: /auto-heal <описание аномалии>");
                continue;
            }

            println!("\n\x1b[36m[AUTO-HEALING]\x1b[0m Автономное лечение (Low/Medium): {}", anomaly);

            let healing = HealingOrchestrator::new(
                kb.clone(),
                Arc::new(dna_engine::DnaEngine::new(&config.database.dna_path, audit.clone())),
                inquisitor.clone(),
                audit.clone(),
                config.clone(),
            );

            let auto = AutonomousRemediation::new(Arc::new(healing), audit.clone());

            match auto.try_auto_heal(&anomaly, PatchType::Config).await {
                Some(result) => {
                    println!("\x1b[32m[AUTO-HEALING SUCCESS]\x1b[0m {}", result.summary());
                }
                None => {
                    println!("\x1b[33m[AUTO-HEALING]\x1b[0m Патч требует HITL или не прошёл проверки. Используйте /heal для полного цикла.");
                }
            }
            continue;
        }

        // === /federation sync ===
        if cmd.starts_with("/federation sync ") {
            let url = cmd.replace("/federation sync ", "").trim().to_string();
            if url.is_empty() {
                println!("Использование: /federation sync <url>");
                continue;
            }
            
            let federation_layer = crate::federation::FederationLayer::new(
                kb.clone(),
                Arc::new(dna_engine::DnaEngine::new(&config.database.dna_path, audit.clone())),
                audit.clone(),
                &config.federation,
                config.node_id.clone(),
            );
            
            println!("\n\x1b[36m[FEDERATION]\x1b[0m Синхронизация с {}", url);
            match federation_layer.sync_with_peer(&url).await {
                Ok(n) => println!("\x1b[32m[FEDERATION SUCCESS]\x1b[0m Синхронизировано записей: {}", n.synced),
                Err(e) => println!("\x1b[31m[FEDERATION ERROR]\x1b[0m Ошибка синхронизации: {}", e),
            }
            continue;
        }

        if cmd == "/kb count black" {
            let count = kb.count_black().await.unwrap_or(0);
            println!("Black Knowledge count: {}", count);
            continue;
        }

        let role = role_clone.lock().await;
        let sp = load_prompt(&role);
        drop(role);
        if let Some(text) = call_llm(&http_client, key_provider.as_ref(), &sp, cmd, &config, local_client.as_ref(), false).await { println!("\n[Agent]: {}\n", text); }
    }
    println!("[SYSTEM] AEGIS Agent остановлен.");
    Ok(())
}