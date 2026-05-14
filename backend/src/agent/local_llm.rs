//! Local LLM Client for Air-Gapped / Hybrid modes.
//!
//! Supports:
//! - Ollama: `http://localhost:11434` with `/api/tags` model listing
//! - vLLM (OpenAI-compatible): `/v1/models`, `/v1/chat/completions`
//!
//! Zero-Trust note: This client never reaches the public internet by itself.

use reqwest::Client;
use serde_json::json;
use std::time::Duration;

#[derive(Clone)]
pub struct LocalLlmClient {
    client: Client,
    base_url: String,
    default_model: String,
}

impl LocalLlmClient {
    pub fn new(base_url: &str, default_model: &str) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    /// Try Ollama first (`/api/tags`), then vLLM (`/v1/models`).
    pub async fn list_models(&self) -> Vec<String> {
        // Ollama
        if let Ok(resp) = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
        {
            if let Ok(v) = resp.json::<serde_json::Value>().await {
                if let Some(models) = v.get("models").and_then(|x| x.as_array()) {
                    let mut out = Vec::new();
                    for m in models {
                        if let Some(name) = m.get("name").and_then(|x| x.as_str()) {
                            out.push(name.to_string());
                        }
                    }
                    if !out.is_empty() {
                        return out;
                    }
                }
            }
        }

        // OpenAI-compatible models listing (vLLM)
        if let Ok(resp) = self
            .client
            .get(format!("{}/v1/models", self.base_url))
            .send()
            .await
        {
            if let Ok(v) = resp.json::<serde_json::Value>().await {
                if let Some(data) = v.get("data").and_then(|x| x.as_array()) {
                    let mut out = Vec::new();
                    for m in data {
                        if let Some(id) = m.get("id").and_then(|x| x.as_str()) {
                            out.push(id.to_string());
                        }
                    }
                    return out;
                }
            }
        }

        vec![]
    }

    /// Main chat method with system, user prompts and optional model override
    pub async fn chat(&self, system: &str, user: &str, model_override: Option<&str>) -> Option<String> {
        let model = model_override.unwrap_or(&self.default_model);
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "temperature": 0.2,
            "max_tokens": 2048
        });

        match self.client.post(url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
                Ok(v) => v
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c0| c0.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string()),
                Err(e) => {
                    tracing::warn!("Local LLM JSON parse failed: {}", e);
                    None
                }
            },
            Ok(resp) => {
                let status = resp.status();
                let err_body = resp.text().await.unwrap_or_default();
                tracing::warn!("Local LLM returned HTTP {}: {}", status, err_body);
                None
            }
            Err(e) => {
                tracing::error!("Local LLM request failed: {}", e);
                None
            }
        }
    }
}
