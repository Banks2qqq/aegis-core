//! High-Interaction Honeypots (Phase 2.2)
//!
//! Создаёт правдоподобные ловушки, которые:
//! - Замедляют атакующего
//! - Собирают TTPs
//! - Автоматически обогащают Black Knowledge + DNA Engine
//!
//! Архитектура:
//! - HoneypotManager — жизненный цикл ловушек
//! - DynamicDeceptionEngine — генерация правдоподобных данных (файлы, БД, API, админки)
//! - InteractionLogger — сбор всех действий атакующего
//! - TTPExtractor — извлечение техник и тактик → сразу в Black Knowledge

use std::sync::{Arc, OnceLock, Weak};

use crate::audit::AuditTrail;
use crate::dna_engine::DnaEngine;
use crate::isolation::{AdaptiveIsolation, IsolationLevel, Workload};
use crate::knowledge::KnowledgeBase;
use crate::knowledge_item::{KnowledgeItem, KnowledgeType};

/// Типы high-interaction honeypots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoneypotType {
    WebAdmin,      // Фейковая админка (типа phpMyAdmin, cPanel)
    Database,      // Ложная БД с правдоподобными данными
    ApiEndpoint,   // API с фейковыми эндпоинтами
    SshShell,      // Полноценный shell (Firecracker + fake FS)
    WindowsShare,  // SMB/CIFS ловушка
}

/// Состояние ловушки.
#[derive(Debug, Clone)]
pub struct HoneypotInstance {
    pub id: String,
    pub htype: HoneypotType,
    pub ip: String,
    pub port: u16,
    pub isolation: IsolationLevel,
    pub started_at: i64,
    pub interactions: u64,
    /// `docker` = nginx container; `memory` = registry only (no listener).
    pub runtime: String,
    pub container_name: String,
    pub canary: String,
}

/// Dynamic Deception Engine — генерирует правдоподобный контент.
pub struct DynamicDeceptionEngine;

impl DynamicDeceptionEngine {
    /// Генерирует уникальный Canary token (для отслеживания утечек и атак).
    pub fn generate_canary_token() -> String {
        let token = uuid::Uuid::new_v4().to_string();
        format!("CANARY-{}", token)
    }

    /// Реально динамическая генерация правдоподобного контента + Canary tokens.
    pub fn generate_fake_content(htype: &HoneypotType) -> String {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let rand_num = rand::random::<u8>() % 100;

        match htype {
            HoneypotType::WebAdmin => {
                format!(
                    r#"<!DOCTYPE html>
<html><head><title>Admin Console v2.4.{}</title></head>
<body>
<h1>Enterprise Admin Panel</h1>
<p>Last login: {}</p>
<p>Server: prod-db-0{}.internal</p>
<form action="/login" method="post">
  Username: <input name="user" value="admin"><br>
  Password: <input type="password" name="pass"><br>
  <button type="submit">Sign In</button>
</form>
<p style="font-size:10px;color:#888">Session ID: {}</p>
</body></html>"#,
                    rand_num,
                    now,
                    rand::random::<u8>() % 5 + 1,
                    &uuid::Uuid::new_v4().to_string()[..8]
                )
            }
            HoneypotType::Database => {
                format!(
                    "CREATE TABLE users (id INT PRIMARY KEY, username VARCHAR(64), password_hash CHAR(32), last_login TIMESTAMP, role VARCHAR(32));\n\
                     INSERT INTO users VALUES (1, 'admin', '5f4dcc3b5aa765d61d8327deb882cf99', '{}', 'superadmin');\n\
                     INSERT INTO users VALUES (2, 'backup', '098f6bcd4621d373cade4e832627b4f6', '{}', 'readonly');\n\
                     INSERT INTO users VALUES (3, 'monitoring', 'd41d8cd98f00b204e9800998ecf8427e', '{}', 'monitor');",
                    now, now, now
                )
            }
            HoneypotType::ApiEndpoint => {
                format!(
                    r#"{{"status":"healthy","version":"2.1.{}","timestamp":"{}","endpoints":["/api/v1/users","/api/v1/config","/api/v1/logs","/api/v1/backup"],"active_sessions":{},"server":"prod-api-0{}"}}"#,
                    rand::random::<u8>() % 50 + 10,
                    now,
                    rand::random::<u8>() % 12 + 3,
                    rand::random::<u8>() % 5 + 1
                )
            }
            HoneypotType::SshShell => {
                format!(
                    "Last login: {} from 10.0.0.{}\nWelcome to Ubuntu 22.04.3 LTS (GNU/Linux 5.15.0-91-generic x86_64)\n$ ",
                    now,
                    rand::random::<u8>() % 200 + 10
                )
            }
            _ => format!("fake_dynamic_content_{}_{}", now, rand_num),
        }
    }
}

/// Interaction Logger — собирает все действия атакующего.
pub struct InteractionLogger {
    audit: Arc<AuditTrail>,
}

impl InteractionLogger {
    pub fn new(audit: Arc<AuditTrail>) -> Self {
        Self { audit }
    }

    pub async fn log_interaction(&self, honeypot_id: &str, action: &str, attacker_ip: &str) {
        let _ = self.audit.log_event(
            "honeypot",
            &format!("interaction hp_id={} action='{}' attacker={}", honeypot_id, action, attacker_ip),
            0.4,
            true,
        );
    }
}

/// Canary Token Tracker — отслеживает срабатывание canary tokens (утечки данных).
pub struct CanaryTracker {
    audit: Arc<AuditTrail>,
}

impl CanaryTracker {
    pub fn new(audit: Arc<AuditTrail>) -> Self {
        Self { audit }
    }

    /// Вызывается, когда canary token был использован (например, в DNS/HTTP запросе).
    pub async fn on_canary_triggered(&self, token: &str, source: &str) {
        let _ = self.audit.log_event(
            "canary",
            &format!("canary_triggered token={} source={}", token, source),
            0.9,
            false,
        );
        tracing::warn!("Canary token triggered: {} from {}", token, source);
    }
}

/// TTP Extractor — извлекает TTPs из логов взаимодействия и отправляет в Black Knowledge.
pub struct TTPExtractor {
    kb: Arc<KnowledgeBase>,
    dna: Arc<DnaEngine>,
}

impl TTPExtractor {
    pub fn new(kb: Arc<KnowledgeBase>, dna: Arc<DnaEngine>) -> Self {
        Self { kb, dna }
    }

    /// Извлекает TTP из логов взаимодействия с помощью базовых эвристик.
    /// В будущем — LLM-based extraction.
    pub async fn extract_and_ingest(&self, honeypot_id: &str, raw_log: &str) -> Result<(), String> {
        let log_lower = raw_log.to_lowercase();
        let mut ttp_tags = vec!["ttp".to_string(), "honeypot".to_string()];
        let mut ttp_summary = "Observed attacker behavior on honeypot".to_string();
        let ttp_id = format!("ttp_{}", &uuid::Uuid::new_v4().to_string()[..8]);

        // === MITRE ATT&CK Inspired Patterns ===

        if log_lower.contains("whoami") || log_lower.contains("id ") || log_lower.contains("uname") || log_lower.contains("hostname") {
            ttp_tags.push("discovery".to_string());
            ttp_summary = "System and user discovery commands executed".to_string();
        }

        if log_lower.contains("cat /etc/passwd") || log_lower.contains("ls /home") || log_lower.contains("getent passwd") {
            ttp_tags.push("credential-access".to_string());
            ttp_summary = "Credential and user enumeration attempted".to_string();
        }

        if log_lower.contains("curl") || log_lower.contains("wget") || log_lower.contains("http") || log_lower.contains("nc ") {
            ttp_tags.push("exfiltration".to_string());
            ttp_tags.push("command-and-control".to_string());
            ttp_summary = "Data exfiltration or C2 communication detected".to_string();
        }

        if log_lower.contains("chmod") || log_lower.contains("chown") || log_lower.contains("sudo") || log_lower.contains("setuid") {
            ttp_tags.push("privilege-escalation".to_string());
            ttp_summary = "Privilege escalation attempt observed".to_string();
        }

        if log_lower.contains("rm -rf") || log_lower.contains("del ") || log_lower.contains("format") {
            ttp_tags.push("impact".to_string());
            ttp_summary = "Destructive or impact activity detected".to_string();
        }

        if log_lower.contains("ssh") || log_lower.contains("scp") || log_lower.contains("sftp") {
            ttp_tags.push("lateral-movement".to_string());
            ttp_summary = "Lateral movement attempt via SSH/SCP".to_string();
        }

        // === Формирование TTP записи ===
        let ttp_content = format!(
            "TTP on honeypot {} [{}]\nSummary: {}\nTags: {:?}\nRaw interaction:\n{}",
            honeypot_id, ttp_id, ttp_summary, ttp_tags, raw_log
        );

        let now = chrono::Utc::now().timestamp();
        let item = KnowledgeItem {
            id: uuid::Uuid::new_v4().to_string(),
            item_type: KnowledgeType::Black,
            content: ttp_content,
            summary: Some(ttp_summary),
            source: format!("honeypot_{}", honeypot_id),
            confidence: 0.82,
            verified_by: vec!["honeypot_manager".to_string()],
            tags: ttp_tags,
            related_iocs: vec![],
            first_seen: now,
            last_seen: now,
            embedding_id: None,
            content_hash: String::new(),
            feedback: None,
        };

        let _ = self.kb.ingest_black(item.clone()).await;
        let _ = self.dna.update_with_items("honeypot_ttp", &[item]).await;

        crate::metrics::honeypot_ttp_extracted();

        Ok(())
    }
}

/// Главный менеджер high-interaction honeypots.
pub struct HoneypotManager {
    instances: tokio::sync::Mutex<Vec<HoneypotInstance>>,
    #[allow(dead_code)]
    deception: DynamicDeceptionEngine,
    logger: InteractionLogger,
    extractor: TTPExtractor,
    canary: CanaryTracker,
    audit: Arc<AuditTrail>,
    pub moving_target: Option<crate::moving_target::MovingTargetDefense>,
    /// Слабая ссылка на себя для фонового `auto_deploy` при срабатывании canary ([`Self::init_canary_escalation`]).
    canary_escalate: OnceLock<Weak<HoneypotManager>>,
}

impl HoneypotManager {
    pub fn new(
        kb: Arc<KnowledgeBase>,
        dna: Arc<DnaEngine>,
        audit: Arc<AuditTrail>,
    ) -> Self {
        Self {
            instances: tokio::sync::Mutex::new(Vec::new()),
            deception: DynamicDeceptionEngine,
            logger: InteractionLogger::new(audit.clone()),
            extractor: TTPExtractor::new(kb, dna),
            canary: CanaryTracker::new(audit.clone()),
            audit,
            moving_target: None,
            canary_escalate: OnceLock::new(),
        }
    }

    /// Связывает менеджер с самим собой для эскалации при canary (один вызов после `Arc::new`).
    pub fn init_canary_escalation(this: &Arc<Self>) {
        let _ = this.canary_escalate.set(Arc::downgrade(this));
    }

    /// Размещает canary для привязки к ловушке (MTD или локальный fallback).
    pub async fn deploy_canary(&self, location: &str) -> Result<String, String> {
        if let Some(mtd) = &self.moving_target {
            mtd.deploy_canary(location).await
        } else {
            let token = DynamicDeceptionEngine::generate_canary_token();
            let _ = self.audit.log_event(
                "honeypots_2.0",
                &format!("canary_deployed location={} token={}", location, token),
                0.4,
                true,
            );
            tracing::info!(
                "Honeypots 2.0: canary deployed at {} -> {}",
                location,
                token
            );
            Ok(token)
        }
    }

    /// Создаёт ловушку: Docker nginx listener (H2) или memory registry fallback.
    pub async fn spawn(&self, htype: HoneypotType, port: u16) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let config = AdaptiveIsolation::for_workload(Workload::Deceiver);
        let data_root = std::env::var("AEGIS_DATA_ROOT")
            .unwrap_or_else(|_| "/opt/aegis/backend/data".into());
        let deception = crate::deception_runtime::DeceptionRuntime::new(&data_root);
        let listener = deception
            .spawn_listener(&id, &htype, port)
            .await?;

        let instance = HoneypotInstance {
            id: id.clone(),
            htype: htype.clone(),
            ip: "127.0.0.1".to_string(),
            port: listener.port,
            isolation: config.level,
            started_at: chrono::Utc::now().timestamp(),
            interactions: 0,
            runtime: listener.runtime.clone(),
            container_name: listener.container_name.clone(),
            canary: listener.canary.clone(),
        };

        tracing::info!(
            "DeceptionRuntime: honeypot id={} type={:?} runtime={} port={} canary={}",
            id,
            instance.htype,
            instance.runtime,
            instance.port,
            instance.canary
        );

        let _ = self.audit.log_event(
            "honeypot_manager",
            &format!(
                "honeypot_spawned id={} type={:?} runtime={} port={} canary={}",
                id, instance.htype, instance.runtime, instance.port, instance.canary
            ),
            0.3,
            true,
        );

        self.instances.lock().await.push(instance);
        Ok(id)
    }

    /// Legacy alias — delegates to [`Self::spawn`] (Docker listener or memory fallback).
    pub async fn spawn_firecracker(&self, htype: HoneypotType, port: u16) -> Result<String, String> {
        self.spawn(htype, port).await
    }

    /// Логирует взаимодействие и пытается извлечь TTP.
    pub async fn record_interaction(&self, honeypot_id: &str, action: &str, attacker_ip: &str, raw_log: &str) {
        self.logger.log_interaction(honeypot_id, action, attacker_ip).await;

        // Пытаемся извлечь TTP после каждого значимого взаимодействия
        if action.len() > 20 {
            let _ = self.extractor.extract_and_ingest(honeypot_id, raw_log).await;
        }
    }

    pub async fn list_active(&self) -> Vec<HoneypotInstance> {
        self.instances.lock().await.clone()
    }

    /// Вызывается при срабатывании canary token (например, из DNS/HTTP ловушки).
    pub async fn on_canary_triggered(&self, token: &str, source: &str) {
        self.canary.on_canary_triggered(token, source).await;
    }

    /// Отслеживание срабатывания Canary tokens (Honeypots 2.0)
    pub async fn track_canary(&self, token: &str, source: &str) -> Result<(), String> {
        let _ = self.audit.log_event(
            "canary",
            &format!(
                "canary_triggered token={} source={} timestamp={}",
                token,
                source,
                chrono::Utc::now().timestamp()
            ),
            0.9,
            false,
        );

        tracing::warn!(
            "Honeypots 2.0: Canary token triggered! token={} source={}",
            token,
            source
        );

        if let Some(weak) = self.canary_escalate.get() {
            if let Some(hm) = weak.upgrade() {
                tokio::spawn(async move {
                    let _ = hm.auto_deploy_honeypots(0.9).await;
                });
            }
        }

        Ok(())
    }

    /// Автономное развёртывание honeypots при обнаружении угрозы
    pub async fn auto_deploy_honeypots(&self, threat_level: f64) -> Result<Vec<String>, String> {
        let mut deployed = Vec::new();

        // Политика развёртывания
        let num_honeypots = if threat_level > 0.8 {
            3 // Высокий риск — 3 ловушки
        } else if threat_level > 0.5 {
            2 // Средний риск — 2 ловушки
        } else {
            1 // Низкий риск — 1 ловушка
        };

        let types = vec![
            HoneypotType::WebAdmin,
            HoneypotType::Database,
            HoneypotType::ApiEndpoint,
            HoneypotType::SshShell,
        ];

        for i in 0..num_honeypots {
            let htype = types[i % types.len()].clone();
            let port = 9000 + (rand::random::<u16>() % 1000); // случайный порт 9000-9999

            match self.spawn(htype.clone(), port).await {
                Ok(id) => {
                    deployed.push(id.clone());
                    tracing::info!(
                        "DeceptionRuntime: auto-deployed {:?} id={} port={}",
                        htype, id, port
                    );
                    if let Ok(token) = self.deploy_canary(&format!("auto_{}", id)).await {
                        tracing::info!(
                            "Honeypots 2.0: Canary token {} registered for honeypot {}",
                            token,
                            id
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("Advanced Deception: failed to deploy honeypot: {}", e);
                }
            }
        }

        let _ = self.audit.log_event(
            "advanced_deception",
            &format!("auto_deployed count={} threat_level={:.2}", deployed.len(), threat_level),
            0.4,
            true,
        );

        Ok(deployed)
    }
}
