//! PR4.3 — LLM / ReAct readiness probe for ops plane.

use std::sync::Arc;

use serde::Serialize;

use crate::config::AEGISConfig;
use crate::key_provider::KeyProvider;
use crate::local_llm::LocalLlmClient;
#[derive(Debug, Clone, Serialize)]
pub struct LlmStatus {
    pub react_ready: bool,
    pub air_gapped: bool,
    pub cloud_available: bool,
    pub local_available: bool,
    pub llm_ready: bool,
    pub cloud_provider: Option<String>,
    pub default_model: Option<String>,
}

pub async fn probe_llm(
    react: Option<&Arc<crate::react_service::ReactService>>,
    config: Option<&Arc<AEGISConfig>>,
    key_provider: Option<&Arc<dyn KeyProvider>>,
    local: Option<&LocalLlmClient>,
) -> LlmStatus {
    let air_gapped = config.map(|c| c.is_air_gapped()).unwrap_or(false);
    let react_ready = react.is_some();
    let cloud_provider = config.and_then(|c| c.llm.cloud_provider.clone());
    let default_model = config.map(|c| c.llm.default_model.clone());

    let cloud_available = if air_gapped {
        false
    } else if let Some(kp) = key_provider {
        kp.get_key("AI_API_KEY").await.is_ok()
    } else {
        std::env::var("AI_API_KEY")
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    };

    let local_available = local.is_some()
        || config
            .map(|c| matches!(c.llm.mode, crate::config::LlmMode::Local | crate::config::LlmMode::Hybrid))
            .unwrap_or(false);

    let llm_ready = if air_gapped {
        local_available
    } else {
        cloud_available || local_available
    };

    LlmStatus {
        react_ready,
        air_gapped,
        cloud_available,
        local_available,
        llm_ready,
        cloud_provider,
        default_model,
    }
}
