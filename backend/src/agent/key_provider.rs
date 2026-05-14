//! KeyProvider abstraction for Zero-Trust secret management.
//!
//! Replaces raw `api_key: String` with a trait so that HSM, Vault, TPM or
//! future providers can be plugged in without touching LLM call sites.
//! Pilot uses EnvKeyProvider (AI_API_KEY / *_API_KEY).

use async_trait::async_trait;
use std::env;

#[derive(Debug, Clone)]
pub enum KeyError {
    NotFound(String),
    VaultError(String),
    Other(String),
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyError::NotFound(name) => write!(f, "KeyError: secret '{}' not found", name),
            KeyError::VaultError(e) => write!(f, "KeyError (Vault): {}", e),
            KeyError::Other(e) => write!(f, "KeyError: {}", e),
        }
    }
}

impl std::error::Error for KeyError {}

#[async_trait]
pub trait KeyProvider: Send + Sync {
    /// Returns the secret for the given logical name (e.g. "AI_API_KEY", "SHODAN_API_KEY").
    async fn get_key(&self, name: &str) -> Result<String, KeyError>;

    /// Convenience: returns the primary LLM API key (AI_API_KEY or OPENAI_API_KEY).
    async fn get_llm_key(&self) -> Result<String, KeyError> {
        self.get_key("AI_API_KEY").await
    }
}

/// Environment-variable based provider (current pilot default).
/// Looks up exact name, then falls back to AI_API_KEY.
pub struct EnvKeyProvider;

#[async_trait]
impl KeyProvider for EnvKeyProvider {
    async fn get_key(&self, name: &str) -> Result<String, KeyError> {
        if let Ok(v) = env::var(name) {
            if !v.trim().is_empty() {
                return Ok(v.trim().to_string());
            }
        }
        // Fallback for LLM calls that historically used AI_API_KEY
        let key = env::var("AI_API_KEY")
            .or_else(|_| env::var("OPENROUTER_API_KEY"))
            .or_else(|_| env::var("OPENAI_API_KEY"))
            .map_err(|_| KeyError::Other(format!("neither {} nor AI_API_KEY/OPENAI_API_KEY set", name)))
            .map(|v| v.trim().to_string());
            
        // Если ключ не найден, возвращаем фиктивный ключ для локального тестирования
        key.or_else(|_| Ok("dummy_key_for_local_testing".to_string()))
    }
}

/// Convenience: static Arc<dyn KeyProvider> for the default pilot provider.
pub fn default_env_provider() -> std::sync::Arc<dyn KeyProvider> {
    std::sync::Arc::new(EnvKeyProvider)
}

/// Vault Key Provider (HashiCorp Vault)
pub struct VaultKeyProvider {
    client: reqwest::Client,
    vault_addr: String,
    token: String,
}

impl VaultKeyProvider {
    pub fn new(vault_addr: &str, token: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            vault_addr: vault_addr.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }
}

#[async_trait]
impl KeyProvider for VaultKeyProvider {
    async fn get_key(&self, name: &str) -> Result<String, KeyError> {
        let url = format!("{}/v1/secret/data/{}", self.vault_addr, name);
        
        let resp = self.client
            .get(&url)
            .header("X-Vault-Token", &self.token)
            .send()
            .await
            .map_err(|e| KeyError::VaultError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(KeyError::NotFound(name.to_string()));
        }

        let json: serde_json::Value = resp.json().await
            .map_err(|e| KeyError::VaultError(e.to_string()))?;

        json["data"]["data"][name]
            .as_str()
            .map(|s| s.to_string())
            .ok_or(KeyError::NotFound(name.to_string()))
    }
}
