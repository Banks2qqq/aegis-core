//! Dashboard ReAct++ — real engine + War Room streaming (PR2).

use std::sync::Arc;

use reqwest::Client;

use crate::audit::AuditTrail;
use crate::config::AEGISConfig;
use crate::critic_agent::CriticAgent;
use crate::key_provider::KeyProvider;
use crate::local_llm::LocalLlmClient;
use crate::mcts::MctsEngine;
use crate::react_engine::{ReactEngine, ReactResult};
use crate::tool_registry::ToolRegistry;
use crate::utils::clip;

fn load_prompt(role: &str) -> String {
    let path = format!("src/agent/prompts/{}.prompt", role);
    std::fs::read_to_string(&path).unwrap_or_else(|_| {
        format!("Твоя роль: {}. Отвечай техническим и кратким языком.", role)
    })
}

/// Shared ReAct runtime for HTTP API (wired from `main.rs`).
pub struct ReactService {
    critic: Arc<CriticAgent>,
    tools: Arc<ToolRegistry>,
    http: Client,
    key_provider: Arc<dyn KeyProvider>,
    local: Option<LocalLlmClient>,
    config: Arc<AEGISConfig>,
    audit: Arc<AuditTrail>,
}

impl ReactService {
    pub fn new(
        critic: Arc<CriticAgent>,
        tools: Arc<ToolRegistry>,
        http: Client,
        key_provider: Arc<dyn KeyProvider>,
        local: Option<LocalLlmClient>,
        config: Arc<AEGISConfig>,
        audit: Arc<AuditTrail>,
    ) -> Self {
        Self {
            critic,
            tools,
            http,
            key_provider,
            local,
            config,
            audit,
        }
    }

    /// Run mission and stream progress to War Room WebSocket.
    pub async fn run_mission(
        &self,
        mission: &str,
        alert_tx: tokio::sync::broadcast::Sender<String>,
    ) -> ReactResult {
        let _ = alert_tx.send(format!("[ReAct++] ▶ Mission: {}", mission));

        if self.config.is_air_gapped() && self.local.is_none() {
            let msg = "[ReAct++] ✗ Air-gapped: local LLM unavailable".to_string();
            let _ = alert_tx.send(msg.clone());
            return ReactResult {
                success: false,
                final_answer: msg,
                observations: vec![],
                iterations_used: 0,
            };
        }

        let has_cloud = self
            .key_provider
            .get_key("AI_API_KEY")
            .await
            .map(|k| !k.trim().is_empty())
            .unwrap_or_else(|_| {
                std::env::var("AI_API_KEY")
                    .map(|k| !k.trim().is_empty())
                    .unwrap_or(false)
            });
        if !has_cloud && self.local.is_none() && !self.config.is_air_gapped() {
            let msg = "[ReAct++] ✗ LLM unavailable (set AI_API_KEY or local LLM)".to_string();
            let _ = alert_tx.send(msg.clone());
            return ReactResult {
                success: false,
                final_answer: msg,
                observations: vec![],
                iterations_used: 0,
            };
        }

        let engine = ReactEngine::new(6)
            .with_mcts(MctsEngine::new(25))
            .with_air_gapped(self.config.is_air_gapped())
            .with_audit(self.audit.clone());

        let http = self.http.clone();
        let kp = self.key_provider.clone();
        let local = self.local.clone();
        let cfg = self.config.clone();

        let result = engine
            .run(
                move |sys, usr| {
                    let http = http.clone();
                    let kp = kp.clone();
                    let local = local.clone();
                    let cfg = cfg.clone();
                    async move {
                        crate::call_llm(&http, kp.as_ref(), &sys, &usr, &cfg, local.as_ref(), true)
                            .await
                    }
                },
                &load_prompt("architect"),
                mission,
                &self.tools,
                &self.critic,
            )
            .await;

        for obs in &result.observations {
            let mark = if obs.success { "✓" } else { "✗" };
            let _ = alert_tx.send(format!(
                "[ReAct++] [{}] {} | {} → {} {}",
                obs.iteration,
                mark,
                clip(&obs.action, 80),
                clip(&obs.result, 160),
                if !obs.thought.is_empty() {
                    format!("| {}", clip(&obs.thought, 60))
                } else {
                    String::new()
                }
            ));
        }

        let status = if result.success { "SUCCESS" } else { "FAILED" };
        let _ = alert_tx.send(format!(
            "[ReAct++] ◼ {} | iterations={}/6 | {}",
            status,
            result.iterations_used,
            clip(&result.final_answer, 300)
        ));

        result
    }

    /// PR4.3 — readiness for `/api/react/status`.
    pub async fn readiness(&self) -> crate::llm_status::LlmStatus {
        let mut status = crate::llm_status::probe_llm(
            None,
            Some(&self.config),
            Some(&self.key_provider),
            self.local.as_ref(),
        )
        .await;
        status.react_ready = true;
        status
    }
}
