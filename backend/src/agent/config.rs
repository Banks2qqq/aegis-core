//! AEGIS Enterprise Configuration Layer
//!
//! Поддерживает 3 источника с приоритетом:
//! 1. CLI arguments (высший приоритет)
//! 2. Environment variables (AEGIS_*)
//! 3. config.yaml
//! 4. Значения по умолчанию
//!
//! Особое внимание уделено Air-Gapped режиму (Zero-Trust).

use clap::Parser;
use figment::{
    providers::{Env, Format, Serialized, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Development,
    Pilot,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmMode {
    Cloud,
    Local,
    Hybrid,
    Airgapped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GodModeSafety {
    Strict,
    AuditOnly,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub mode: LlmMode,
    pub cloud_provider: Option<String>,
    pub local_base_url: Option<String>,
    pub fallback_enabled: bool,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub air_gapped: bool,
    pub human_in_the_loop: bool,
    pub god_mode_safety_level: GodModeSafety,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub sqlite_path: String,
    pub qdrant_url: String,
    /// Путь к DNA-снимку (JSON). По умолчанию: ./data/aegis_dna.json
    #[serde(default = "default_dna_path")]
    pub dna_path: String,
}

fn default_dna_path() -> String {
    "./data/aegis_dna.json".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enabled: bool,
    #[serde(alias = "immutable_log_path")]
    pub log_path: String,
    pub immutable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub enabled: bool,
    pub address: String,
    pub token: String,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            address: "http://127.0.0.1:8200".to_string(),
            token: "".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationPeerConfig {
    pub id: String,
    /// Public HTTPS base (health, dashboard). Default :443.
    pub url: String,
    /// Federation mTLS listener (e.g. `:8443`). Falls back to `url` when unset.
    #[serde(default)]
    pub federation_url: Option<String>,
    /// Optional per-peer token (overrides federation.shared_secret).
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default)]
    pub mtls: bool,
}

impl FederationPeerConfig {
    pub fn health_base(&self) -> String {
        self.url.trim_end_matches('/').to_string()
    }

    pub fn federation_base(&self) -> String {
        self.federation_url
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| self.health_base())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FederationConfig {
    /// This node's public base URL (for peer configs pointing here).
    #[serde(default)]
    pub public_url: Option<String>,
    /// Node-to-node auth; falls back to env `FEDERATION_SHARED_SECRET`.
    #[serde(default)]
    pub shared_secret: Option<String>,
    /// Background sync interval; 0 = disabled.
    #[serde(default)]
    pub sync_interval_secs: u64,
    #[serde(default)]
    pub mtls_client_cert: Option<String>,
    #[serde(default)]
    pub mtls_client_key: Option<String>,
    #[serde(default)]
    pub mtls_ca_cert: Option<String>,
    #[serde(default)]
    pub peers: Vec<FederationPeerConfig>,
}

impl FederationConfig {
    /// Local federation mTLS listener (default :8443 on same host as `public_url`).
    pub fn federation_listen_url(&self) -> Option<String> {
        let base = self.public_url.as_ref()?.trim();
        if base.is_empty() {
            return None;
        }
        let base = base.trim_end_matches('/');
        if base.contains(":8443") {
            return Some(base.to_string());
        }
        Some(format!("{base}:8443"))
    }

    pub fn effective_shared_secret(&self) -> Option<String> {
        if let Some(s) = &self.shared_secret {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        std::env::var("FEDERATION_SHARED_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// All valid inbound/outbound federation tokens (global + per-peer).
    pub fn accepted_tokens(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(s) = self.effective_shared_secret() {
            out.push(s);
        }
        for p in &self.peers {
            if let Some(t) = &p.auth_token {
                let t = t.trim();
                if !t.is_empty() && !out.iter().any(|x| x == t) {
                    out.push(t.to_string());
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AEGISConfig {
    #[serde(default = "default_node_id")]
    pub node_id: String,
    pub mode: RunMode,
    pub llm: LlmConfig,
    pub security: SecurityConfig,
    pub database: DatabaseConfig,
    pub audit: AuditConfig,
    #[serde(default)]
    pub vault: VaultConfig,
    #[serde(default)]
    pub federation: FederationConfig,
}

fn default_node_id() -> String {
    "node_default".to_string()
}

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
#[command(name = "aegis", about = "AEGIS Enterprise - Zero-Trust Autonomous Defense")]
pub struct CliArgs {
    /// Path to config file
    #[arg(long, default_value = "config.yaml")]
    pub config: String,

    /// Override LLM mode (cloud|local|hybrid|airgapped)
    #[arg(long)]
    pub llm_mode: Option<String>,

    /// Force air-gapped mode (disables all external calls)
    #[arg(long)]
    pub air_gapped: bool,

    /// Run mode (development|pilot|production)
    #[arg(long)]
    pub mode: Option<String>,
}

impl Default for AEGISConfig {
    fn default() -> Self {
        Self {
            node_id: "node_default".to_string(),
            mode: RunMode::Development,
            llm: LlmConfig {
                mode: LlmMode::Local,
                cloud_provider: Some("openrouter".to_string()),
                local_base_url: Some("http://localhost:11434/v1".to_string()),
                fallback_enabled: true,
                default_model: "qwen2:14b-instruct-q5_K_M".to_string(),
            },
            security: SecurityConfig {
                air_gapped: false,
                human_in_the_loop: true,
                god_mode_safety_level: GodModeSafety::Strict,
            },
            database: DatabaseConfig {
                sqlite_path: "./data/aegis.db".to_string(),
                // Qdrant REST (HTTP) defaults to 6333; 6334 is gRPC.
                qdrant_url: "http://localhost:6333".to_string(),
                dna_path: "./data/aegis_dna.json".to_string(),
            },
            audit: AuditConfig {
                enabled: true,
                log_path: "./data/audit.log".to_string(),
                immutable: true,
            },
            vault: VaultConfig::default(),
            federation: FederationConfig::default(),
        }
    }
}

impl AEGISConfig {
    /// Загружает конфигурацию из всех источников с правильным приоритетом
    pub fn load() -> Result<Self, figment::Error> {
        let args = CliArgs::parse();

        let figment = Figment::from(Serialized::defaults(AEGISConfig::default()))
            .merge(Yaml::file(&args.config))
            .merge(Env::prefixed("AEGIS_").split("_"));

        let mut config: AEGISConfig = figment.extract()?;

        // CLI override имеет высший приоритет
        if let Some(mode) = config_from_cli_llm_mode() {
            config.llm.mode = match mode.as_str() {
                "cloud" => LlmMode::Cloud,
                "local" => LlmMode::Local,
                "hybrid" => LlmMode::Hybrid,
                "airgapped" => LlmMode::Airgapped,
                _ => config.llm.mode,
            };
        }

        if config_from_cli_air_gapped() {
            config.security.air_gapped = true;
            config.llm.mode = LlmMode::Airgapped;
        }

        if let Some(mode) = config_from_cli_run_mode() {
            config.mode = match mode.as_str() {
                "development" => RunMode::Development,
                "pilot" => RunMode::Pilot,
                "production" => RunMode::Production,
                _ => config.mode,
            };
        }

        Ok(config)
    }

    /// Проверяет, находится ли система в Air-Gapped режиме
    pub fn is_air_gapped(&self) -> bool {
        self.security.air_gapped || self.llm.mode == LlmMode::Airgapped
    }

    /// Возвращает текущий LLM режим
    pub fn get_llm_mode(&self) -> LlmMode {
        if self.is_air_gapped() {
            LlmMode::Airgapped
        } else {
            self.llm.mode.clone()
        }
    }

    /// Валидация критических параметров
    pub fn validate(&self) -> Result<(), String> {
        if self.is_air_gapped() && self.llm.local_base_url.is_none() {
            return Err("Air-gapped mode requires local_base_url to be set".to_string());
        }

        if matches!(self.mode, RunMode::Production)
            && self.security.god_mode_safety_level == GodModeSafety::Disabled
        {
            return Err("Production mode cannot have god_mode_safety_level=disabled".to_string());
        }

        Ok(())
    }

    /// Возвращает предупреждение при запуске в Air-Gapped режиме
    pub fn airgap_warning(&self) -> Option<String> {
        if self.is_air_gapped() {
            Some(
                "⚠️  AIR-GAPPED MODE ACTIVE — All external LLM and threat intel calls are DISABLED"
                    .to_string(),
            )
        } else {
            None
        }
    }
}

// ---- CLI helpers (parse once-per-process) ----
fn cli_args_once() -> &'static CliArgs {
    use std::sync::OnceLock;
    static ARGS: OnceLock<CliArgs> = OnceLock::new();
    ARGS.get_or_init(CliArgs::parse)
}

fn config_from_cli_llm_mode() -> Option<String> {
    cli_args_once().llm_mode.clone()
}

fn config_from_cli_air_gapped() -> bool {
    cli_args_once().air_gapped
}

fn config_from_cli_run_mode() -> Option<String> {
    cli_args_once().mode.clone()
}

