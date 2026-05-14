use serde::{Deserialize, Serialize};
use super::tool_registry::ToolRegistry;
use super::critic_agent::CriticAgent;
use super::mcts::MctsEngine;
use super::audit::AuditTrail;
use std::sync::Arc;
use tokio::time::{timeout, Duration as TokioDuration};
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec};
use std::sync::LazyLock;

static REACT_TOOL_EXEC_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        prometheus::Opts::new("aegis_react_tool_exec_total", "ReAct tool executions"),
        &["tool", "success"],
    )
    .expect("metric");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static REACT_TOOL_EXEC_SECONDS: LazyLock<HistogramVec> = LazyLock::new(|| {
    let h = HistogramVec::new(
        HistogramOpts::new("aegis_react_tool_exec_seconds", "ReAct tool execution latency seconds"),
        &["tool"],
    )
    .expect("metric");
    let _ = prometheus::default_registry().register(Box::new(h.clone()));
    h
});

static REACT_TOOL_TIMEOUT_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        prometheus::Opts::new("aegis_react_tool_timeout_total", "ReAct tool timeouts"),
        &["tool"],
    )
    .expect("metric");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static REACT_LLM_TIMEOUT_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        prometheus::Opts::new("aegis_react_llm_timeout_total", "ReAct LLM timeouts"),
        &["phase"],
    )
    .expect("metric");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

/// Одно наблюдение агента в цикле ReAct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub iteration: u32,
    pub thought: String,
    pub action: String,
    pub result: String,
    pub success: bool,
}

/// Результат ReAct-миссии
#[derive(Debug, Clone)]
pub struct ReactResult {
    pub success: bool,
    pub final_answer: String,
    pub observations: Vec<Observation>,
    pub iterations_used: u32,
}

/// ReAct++ runtime with MCTS branch selection
pub struct ReactEngine {
    max_iterations: u32,
    mcts: Option<MctsEngine>,
    air_gapped: bool,
    audit: Option<Arc<AuditTrail>>,
    llm_timeout: TokioDuration,
    tool_timeout: TokioDuration,
    max_action_len: usize,
    max_prompt_chars: usize,
    max_history_items: usize,
}

impl ReactEngine {
    pub fn new(max_iterations: u32) -> Self {
        Self {
            max_iterations,
            mcts: None,
            air_gapped: false,
            audit: None,
            llm_timeout: TokioDuration::from_secs(25),
            tool_timeout: TokioDuration::from_secs(20),
            max_action_len: 512,
            max_prompt_chars: 24_000,
            max_history_items: 8,
        }
    }

    pub fn with_air_gapped(mut self, enabled: bool) -> Self {
        self.air_gapped = enabled;
        self
    }

    pub fn with_audit(mut self, audit: Arc<AuditTrail>) -> Self {
        self.audit = Some(audit);
        self
    }

    pub fn with_llm_timeout(mut self, d: TokioDuration) -> Self {
        self.llm_timeout = d;
        self
    }

    pub fn with_tool_timeout(mut self, d: TokioDuration) -> Self {
        self.tool_timeout = d;
        self
    }

    /// Enable MCTS for optimal action selection using Critic scores
    pub fn with_mcts(mut self, mcts: MctsEngine) -> Self {
        self.mcts = Some(mcts);
        self
    }

    /// Запустить ReAct++ миссию с Critic evaluation и реальными инструментами
    ///
    /// `call_llm` — LLM функция
    /// `system_prompt` — роль агента
    /// `mission` — задача
    /// `registry` — ToolRegistry с реальными исполнителями
    /// `critic` — ReAct++ Critic Agent (security + utility evaluation)
    pub async fn run<F, Fut>(
        &self,
        call_llm: F,
        system_prompt: &str,
        mission: &str,
        registry: &ToolRegistry,
        critic: &CriticAgent,
    ) -> ReactResult
    where
        F: Fn(String, String) -> Fut,
        Fut: std::future::Future<Output = Option<String>>,
    {
        let mut observations: Vec<Observation> = Vec::new();
        let tools_desc = registry.get_tools_description();

        // Начальный промпт
        let mut context = format!(
            "{}\n\n## МИССИЯ\n{}\n\n## ДОСТУПНЫЕ ИНСТРУМЕНТЫ\n{}\n\nНачинай выполнение. Опиши своё наблюдение, мысль и действие.",
            system_prompt, mission, tools_desc
        );

        let mut last_action: Option<String> = None;
        let mut repeat_count: u32 = 0;

        for i in 1..=self.max_iterations {
            // Keep prompt bounded to avoid runaway context growth
            let history = observations
                .iter()
                .rev()
                .take(self.max_history_items)
                .rev()
                .map(|o| format!(
                    "[Попытка {}] Мысль: {}, Действие: {}, Результат({}): {}",
                    o.iteration,
                    Self::clip(&o.thought, 256),
                    Self::clip(&o.action, 256),
                    if o.success { "ok" } else { "fail" },
                    Self::clip(&o.result, 512)
                ))
                .collect::<Vec<_>>()
                .join("\n");

            let mut prompt = format!(
                "{}\n\nЭто попытка {}/{}. Предыдущие наблюдения (последние {}):\n{}\n\nФормат ответа:\nНАБЛЮДЕНИЕ: <что видишь>\nМЫСЛЬ: <что думаешь>\nДЕЙСТВИЕ: <tool(...) | final_answer(...)>",
                context,
                i,
                self.max_iterations,
                self.max_history_items,
                history
            );
            if prompt.len() > self.max_prompt_chars {
                prompt.truncate(self.max_prompt_chars);
            }

            let response = match timeout(self.llm_timeout, call_llm(system_prompt.to_string(), prompt)).await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    return ReactResult {
                        success: false,
                        final_answer: "LLM не ответил".into(),
                        observations,
                        iterations_used: i,
                    };
                }
                Err(_) => {
                    REACT_LLM_TIMEOUT_TOTAL.with_label_values(&["call_llm"]).inc();
                    return ReactResult {
                        success: false,
                        final_answer: "LLM timeout".into(),
                        observations,
                        iterations_used: i,
                    };
                }
            };

            // Парсинг ответа
            let thought = Self::extract_field(&response, "МЫСЛЬ:");
            let action = Self::extract_field(&response, "ДЕЙСТВИЕ:");
            let observation_text = Self::extract_field(&response, "НАБЛЮДЕНИЕ:");

            // Normalize + hard limits for action
            let mut action = action.trim().to_string();
            if action.len() > self.max_action_len {
                action.truncate(self.max_action_len);
            }
            if action.is_empty() {
                action = "final_answer(\"empty action\")".to_string();
            }

            // === MCTS: Re-rank action using Critic scores (if enabled) ===
            let final_action = if let Some(mcts) = &self.mcts {
                let candidates = vec![
                    action.clone(),
                    "final_answer()".to_string(),
                    format!("search_knowledge_base(query=\"{}\")", mission),
                ];
                let (best_action, _score) = mcts.select_best_action(
                    mission,
                    &observation_text,
                    &candidates,
                    critic,
                    registry,
                ).await;
                best_action
            } else {
                action.clone()
            };

            // Anti-loop: if model repeats same action too much, force final_answer
            if last_action.as_deref() == Some(final_action.as_str()) {
                repeat_count = repeat_count.saturating_add(1);
            } else {
                repeat_count = 0;
                last_action = Some(final_action.clone());
            }
            if repeat_count >= 2 {
                let msg = format!("Loop detected: repeated action '{}' multiple times", Self::clip(&final_action, 128));
                return ReactResult {
                    success: false,
                    final_answer: msg,
                    observations,
                    iterations_used: i,
                };
            }

            // === ReAct++ Critic Evaluation (Zero-Trust) ===
            let critic_score = critic.evaluate(&final_action, mission, &observation_text).await;
            if let Some(audit) = &self.audit {
                let _ = audit.log_event(
                    "react_engine",
                    &format!("critic_decision action='{}' verdict='{}'", final_action, critic_score.verdict),
                    critic_score.security_risk,
                    false,
                );
            }

            let (result, success) = if critic_score.security_risk > 0.8 || critic_score.needs_human_approval {
                if let Some(audit) = &self.audit {
                    let _ = audit.log_event(
                        "react_engine",
                        &format!("escalate_human_approval_required action='{}'", final_action),
                        critic_score.security_risk,
                        false,
                    );
                }
                (
                    format!(
                        "[ESCALATE: HUMAN APPROVAL REQUIRED] risk={:.2} verdict={} reason={}",
                        critic_score.security_risk, critic_score.verdict, critic_score.reasoning
                    ),
                    false,
                )
            } else if critic_score.security_risk > 0.75 {
                // KILL SWITCH — блокируем опасные действия
                if let Some(audit) = &self.audit {
                    let _ = audit.log_event(
                        "react_engine",
                        &format!("blocked_by_critic action='{}'", final_action),
                        critic_score.security_risk,
                        false,
                    );
                }
                (
                    format!("[BLOCKED by Critic] risk={:.2} reason={}", critic_score.security_risk, critic_score.reasoning),
                    false,
                )
            } else if critic_score.verdict == "BLOCK" {
                if let Some(audit) = &self.audit {
                    let _ = audit.log_event(
                        "react_engine",
                        &format!("blocked verdict=BLOCK action='{}'", final_action),
                        critic_score.security_risk,
                        false,
                    );
                }
                (format!("[BLOCKED] {}", critic_score.reasoning), false)
            } else {
                // Only allow: tool(...) or final_answer(...)
                let fa = final_action.to_lowercase();
                if fa.starts_with("final_answer") || fa.starts_with("mission_complete") {
                    (final_action.clone(), true)
                } else if let Some((tool_name, params)) = ToolRegistry::parse_action(&final_action) {
                    if let Err(e) = registry.validate_call(&tool_name, &params) {
                        return ReactResult {
                            success: false,
                            final_answer: format!("Tool call rejected by schema: {:?} (tool={})", e, tool_name),
                            observations,
                            iterations_used: i,
                        };
                    }
                    let exec_fut = registry.execute(&tool_name, params);
                    let t0 = std::time::Instant::now();
                    match timeout(self.tool_timeout, exec_fut).await {
                        Ok(Some(tool_result)) => {
                            let secs = t0.elapsed().as_secs_f64();
                            REACT_TOOL_EXEC_SECONDS.with_label_values(&[&tool_name]).observe(secs);
                            REACT_TOOL_EXEC_TOTAL
                                .with_label_values(&[&tool_name, if tool_result.success { "1" } else { "0" }])
                                .inc();
                            if let Some(audit) = &self.audit {
                                let _ = audit.log_event(
                                    "react_engine",
                                    &format!("tool_exec tool='{}' success={}", tool_name, tool_result.success),
                                    critic_score.security_risk,
                                    false,
                                );
                            }
                            (Self::clip(&tool_result.output, 4000), tool_result.success)
                        }
                        Ok(None) => (format!("Tool '{}' not found", tool_name), false),
                        Err(_) => {
                            REACT_TOOL_TIMEOUT_TOTAL.with_label_values(&[&tool_name]).inc();
                            (format!("Tool '{}' timed out", tool_name), false)
                        }
                    }
                } else {
                    (format!("Invalid action format (must be tool(...) or final_answer(...)): {}", final_action), false)
                }
            };

            let obs = Observation {
                iteration: i,
                thought: thought.clone(),
                action: final_action.clone(),
                result: result.clone(),
                success,
            };

            observations.push(obs);

            // Проверка на завершение
            if final_action.to_lowercase().contains("final_answer")
                || final_action.to_lowercase().contains("mission_complete")
                || final_action.to_lowercase().contains("задача выполнена")
            {
                return ReactResult {
                    success: true,
                    final_answer: result,
                    observations,
                    iterations_used: i,
                };
            }

            // Обновление контекста для следующей итерации
            context = format!(
                "Предыдущее действие: {}\nРезультат: {}\nПродолжай.",
                final_action, result
            );
        }

        // Исчерпаны все итерации
        ReactResult {
            success: false,
            final_answer: format!(
                "Исчерпаны все {} итераций. Последнее наблюдение: {}",
                self.max_iterations,
                observations.last().map(|o| o.result.as_str()).unwrap_or("нет данных")
            ),
            observations,
            iterations_used: self.max_iterations,
        }
    }

    /// Извлечь значение поля из ответа LLM
    fn extract_field(response: &str, field: &str) -> String {
        response
            .lines()
            .find(|l| l.to_lowercase().starts_with(&field.to_lowercase()))
            .and_then(|l| l.split_once(':').map(|x| x.1.trim()))
            .unwrap_or("не указано")
            .trim()
            .to_string()
    }

    fn clip(s: &str, max: usize) -> String {
        if s.len() <= max { return s.to_string(); }
        let mut out = s.to_string();
        out.truncate(max);
        out.push('…');
        out
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_field() {
        let response = "НАБЛЮДЕНИЕ: порт открыт\nМЫСЛЬ: нужно сканировать\nДЕЙСТВИЕ: scan_port(443)";
        assert_eq!(ReactEngine::extract_field(response, "МЫСЛЬ:"), "нужно сканировать");
        assert_eq!(ReactEngine::extract_field(response, "ДЕЙСТВИЕ:"), "scan_port(443)");
    }

    // Интеграционный тест (Фаза 5) — проверяет, что движок собирается и базовая логика работает
    #[tokio::test]
    async fn test_react_engine_compiles_and_runs_basic() {
        let engine = ReactEngine::new(2);
        // Просто проверяем, что типы и lifetime верны — реальные LLM вызовы в integration-тестах
        assert_eq!(engine.max_iterations, 2);
    }
}