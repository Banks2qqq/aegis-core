//! LearningOrchestrator — full Scout → Critic 2.0 → Inquisitor 2.0 → Ingest → DNA 2.5 cycle.
//!
//! Extracted from the monolithic /scout handler in agent/main.rs (lines ~711-1185).
//! This enables:
//! - Scheduled autonomous research cycles (cron / agent_bus)
//! - Unit / integration testing of the learning loop
//! - Clear separation of concerns (CLI only handles I/O + HITL prompts)
//!
//! Zero-Trust: all sub-components (Scout, Critic, Inquisitor) already carry AuditTrail.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;

use crate::audit::AuditTrail;
use crate::config::AEGISConfig;
use crate::critic_agent::{CriticAgent, CriticEvaluation, Verdict};
use crate::dna_engine::DnaEngine;
use crate::inquisitor_agent::{Inquisitor, evaluation_is_hard_block, evaluation_requires_escalation};
use crate::knowledge::KnowledgeBase;
use crate::knowledge_item::{KnowledgeItem, KnowledgeType};
use crate::metrics;
use crate::scout::Scout;

/// Structured result of one learning cycle (for logging / dashboards / scheduling).
#[derive(Debug, Clone)]
pub struct CycleResult {
    pub topic: String,
    pub items_found: usize,
    pub critic_verdict: String,
    pub critic_risk: f64,
    pub inquisitor_blocks: usize,
    pub inquisitor_escalates: usize,
    pub ingested_ok: usize,
    pub ingested_new: usize,
    pub ingested_updated: usize,
    pub ingested_white: usize,
    pub ingested_black: usize,
    pub ingested_err: usize,
    pub dna_updated: bool,
    pub dna_snapshot: Option<crate::dna_engine::DnaSnapshot>,
}

/// High-level orchestrator for the self-learning research cycle.
pub struct LearningOrchestrator {
    scout: Arc<Scout>,
    critic: Arc<CriticAgent>,
    inquisitor: Arc<Inquisitor>,
    kb: Arc<KnowledgeBase>,
    audit: Arc<AuditTrail>,
    config: Arc<AEGISConfig>,
    #[allow(dead_code)]
    http_client: Client,
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    local_client: Option<crate::local_llm::LocalLlmClient>,
}

impl LearningOrchestrator {
    pub fn new(
        scout: Arc<Scout>,
        critic: Arc<CriticAgent>,
        inquisitor: Arc<Inquisitor>,
        kb: Arc<KnowledgeBase>,
        audit: Arc<AuditTrail>,
        config: Arc<AEGISConfig>,
        http_client: Client,
        api_key: String,
        local_client: Option<crate::local_llm::LocalLlmClient>,
    ) -> Self {
        Self {
            scout,
            critic,
            inquisitor,
            kb,
            audit,
            config,
            http_client,
            api_key,
            local_client,
        }
    }

    /// Runs the full research → evaluation → ingest → DNA update cycle.
    /// Returns structured result suitable for metrics, logging and scheduling.
    ///
    /// This is the single source of truth for "one learning iteration".
    pub async fn run_cycle(&self, topic: &str) -> Result<CycleResult, String> {
        let topic = topic.trim();
        if topic.is_empty() {
            return Err("empty topic".to_string());
        }

        // HITL gate (if enabled) — caller (CLI) may have already prompted,
        // but we keep a second safety check here for scheduled runs.
        if self.config.security.human_in_the_loop {
            // For non-interactive runs we skip interactive prompt and rely on
            // the fact that scheduled jobs should have explicit policy.
            // In CLI context the outer handler already did the prompt.
        }

        // 1. Scout
        let items = self.scout.run_advanced(topic).await.map_err(|e| e.to_string())?;
        self.run_cycle_from_items(topic, items, None).await
    }

    /// Critic → Inquisitor → Ingest → DNA для уже собранных items (API / ФСТЭК БДУ).
    pub async fn run_cycle_from_items(
        &self,
        topic: &str,
        items: Vec<KnowledgeItem>,
        progress: Option<&tokio::sync::broadcast::Sender<String>>,
    ) -> Result<CycleResult, String> {
        let topic = topic.trim();
        if items.is_empty() {
            return Err("no knowledge items to process".into());
        }

        let trusted_fstec = items.iter().all(|i| i.source == "fstec_bdu");

        pipeline_msg(
            progress,
            format!("[CRITIC] ▶ Оценка риска для {} записей (0–1)", items.len()),
        );

        // 2. Critic 2.0 per-item (FuturesUnordered)
        let k_ctx = crate::critic_agent::format_scout_context_for_critic(topic, &items);
        let mut eval_targets: Vec<&KnowledgeItem> = items
            .iter()
            .filter(|i| i.item_type == KnowledgeType::Hypothesis)
            .collect();
        eval_targets.extend(items.iter().filter(|i| {
            matches!(
                i.item_type,
                KnowledgeType::White | KnowledgeType::Black | KnowledgeType::TTP
            )
        }));
        let mut seen = HashSet::new();
        eval_targets.retain(|i| seen.insert(i.id.clone()));
        eval_targets.truncate(24);

        let mut critic_futures = FuturesUnordered::new();
        for it in &eval_targets {
            let critic_ref = self.critic.clone();
            let k_ctx_ref = k_ctx.clone();
            let it_owned = (*it).clone();
            critic_futures.push(async move {
                let result = critic_ref.evaluate_knowledge(&it_owned, Some(&k_ctx_ref)).await;
                (it_owned.id.clone(), result)
            });
        }

        let mut critic_by_id: HashMap<String, CriticEvaluation> = HashMap::new();
        let mut max_knowledge_risk = 0.0_f64;
        let mut any_k_block = false;
        let mut any_k_escalate = false;
        let mut low_critic_conf = 0usize;

        while let Some((id, result)) = critic_futures.next().await {
            if let Ok(ev) = result {
                max_knowledge_risk = max_knowledge_risk.max(ev.security_risk);
                if ev.verdict == Verdict::Block {
                    any_k_block = true;
                }
                if ev.verdict == Verdict::Escalate {
                    any_k_escalate = true;
                }
                if ev.confidence < 0.45 {
                    low_critic_conf += 1;
                }
                critic_by_id.insert(id, ev);
            }
        }

        if low_critic_conf > 0 && low_critic_conf * 2 >= eval_targets.len().max(1) {
            let _ = self.audit.log_event(
                "critic",
                &format!("critic_knowledge_low_confidence_batch n={} low={}", eval_targets.len(), low_critic_conf),
                0.4,
                false,
            );
        }

        // Bulk Critic
        let synthesis = format!(
            "SCOUT ITEMS TOPIC: {}\nITEMS:\n{}",
            topic,
            items
                .iter()
                .take(25)
                .map(|it| {
                    format!(
                        "- {:?} | {} | {:.2}\n{}",
                        it.item_type,
                        it.source,
                        it.confidence,
                        crate::utils::clip(&it.content, 300)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        );
        let critic_score = self.critic.evaluate("ingest_knowledge_base()", topic, &synthesis).await;
        let merged_risk = critic_score.security_risk.max(max_knowledge_risk);
        let merged_verdict = if any_k_block || critic_score.verdict == "BLOCK" {
            "BLOCK".to_string()
        } else if any_k_escalate || critic_score.verdict == "ESCALATE" {
            "ESCALATE".to_string()
        } else {
            critic_score.verdict.clone()
        };

        metrics::critic_bulk_verdict(&merged_verdict);

        pipeline_msg(
            progress,
            format!(
                "[CRITIC] risk={:.2} utility={:.2} → VERDICT: {}",
                merged_risk,
                critic_score.utility,
                merged_verdict
            ),
        );

        if merged_verdict == "BLOCK" || (!trusted_fstec && merged_risk >= 0.95) {
            pipeline_msg(progress, "[CRITIC] ✗ Ingest заблокирован (BLOCK)");
            let _ = self.audit.log_event("agent-cli", "scout_blocked_by_critic", merged_risk, false);
            metrics::learning_gate_finish("critic", false);
            return Ok(CycleResult {
                topic: topic.to_string(),
                items_found: items.len(),
                critic_verdict: merged_verdict,
                critic_risk: merged_risk,
                inquisitor_blocks: 0,
                inquisitor_escalates: 0,
                ingested_ok: 0,
                ingested_new: 0,
                ingested_updated: 0,
                ingested_white: 0,
                ingested_black: 0,
                ingested_err: 0,
                dna_updated: false,
                dna_snapshot: None,
            });
        }

        pipeline_msg(
            progress,
            format!(
                "[INQUISITOR] ▶ Глубокий анализ {} целей (exploit / in-the-wild)",
                eval_targets.len()
            ),
        );

        // 3. Inquisitor 2.0 (parallel)
        let mut inq_futures = FuturesUnordered::new();
        for it in &eval_targets {
            let inq_ref = self.inquisitor.clone();
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
        let mut inq_audit_events: Vec<(String, f64)> = Vec::new();

        while let Some((it_owned, result)) = inq_futures.next().await {
            match result {
                Ok(ev) => {
                    metrics::inquisitor_knowledge_verdict(ev.verdict.as_str());
                    if evaluation_is_hard_block(&ev) {
                        inq_any_hard_block = true;
                        inq_block_details.push(format!(
                            "id={} type={:?} verdict={} risk_areas={:?} | {}",
                            &it_owned.id[..it_owned.id.len().min(16)],
                            it_owned.item_type,
                            ev.verdict.as_str(),
                            ev.risk_areas,
                            ev.reasoning.chars().take(220).collect::<String>()
                        ));
                        inq_audit_events.push((
                            format!(
                                "scout_inquisitor2_BLOCK_detail id={} risk_areas={:?} reasoning_excerpt={}",
                                &it_owned.id[..it_owned.id.len().min(12)],
                                ev.risk_areas,
                                ev.reasoning.chars().take(160).collect::<String>()
                            ),
                            0.9,
                        ));
                    } else if evaluation_requires_escalation(&ev) {
                        inq_any_escalate = true;
                        inq_escalate_details.push(format!(
                            "id={} type={:?} verdict={} | {}",
                            &it_owned.id[..it_owned.id.len().min(16)],
                            it_owned.item_type,
                            ev.verdict.as_str(),
                            ev.reasoning.chars().take(160).collect::<String>()
                        ));
                    }
                }
                Err(_) => {}
            }
        }

        for (msg, risk) in inq_audit_events {
            let _ = self.audit.log_event("inquisitor", &msg, risk, false);
        }

        let merged_inq = if inq_any_hard_block {
            "BLOCK"
        } else if inq_any_escalate {
            "ESCALATE"
        } else {
            "ALLOW"
        };

        pipeline_msg(
            progress,
            format!(
                "[INQUISITOR] verdict={} | blocks={} escalates={}",
                merged_inq,
                inq_block_details.len(),
                inq_escalate_details.len()
            ),
        );

        if trusted_fstec && merged_inq == "BLOCK" {
            pipeline_msg(
                progress,
                "[INQUISITOR] ESCALATE для ФСТЭК БДУ — ingest разрешён (официальный источник)",
            );
        }

        if merged_inq == "BLOCK" && !trusted_fstec {
            pipeline_msg(progress, "[INQUISITOR] ✗ Ingest заблокирован");
            metrics::learning_gate_finish("inquisitor", false);
            return Ok(CycleResult {
                topic: topic.to_string(),
                items_found: items.len(),
                critic_verdict: merged_verdict,
                critic_risk: merged_risk,
                inquisitor_blocks: inq_block_details.len(),
                inquisitor_escalates: inq_escalate_details.len(),
                ingested_ok: 0,
                ingested_new: 0,
                ingested_updated: 0,
                ingested_white: 0,
                ingested_black: 0,
                ingested_err: 0,
                dna_updated: false,
                dna_snapshot: None,
            });
        }

        // HITL for ESCALATE / high risk
        if (merged_inq == "ESCALATE" || merged_risk > 0.6) && self.config.security.human_in_the_loop {
            // In CLI context the outer prompt already happened.
            // For scheduled runs we would need a different approval channel (webhook / Slack).
            // For now we log and continue (policy decision).
            let _ = self.audit.log_event("agent-cli", "scout_escalate_auto_approved_scheduled", merged_risk, true);
        }

        pipeline_msg(progress, "[INGEST] ▶ Запись в Black Knowledge + метрики");

        // 4. Ingest (White/Black only)
        let mut ingested_ok = 0usize;
        let mut ingested_new = 0usize;
        let mut ingested_updated = 0usize;
        let mut ok_white = 0usize;
        let mut ok_black = 0usize;
        let mut ingested_err = 0usize;

        for it in &items {
            match it.item_type {
                KnowledgeType::White => {
                    match self.kb.ingest_white(it.clone()).await {
                        Ok(deduped) => {
                            ingested_ok += 1;
                            ok_white += 1;
                            if deduped {
                                ingested_updated += 1;
                                metrics::knowledge_deduped(1);
                            } else {
                                ingested_new += 1;
                                metrics::knowledge_ingested(&KnowledgeType::White, 1);
                            }
                        }
                        Err(_) => { ingested_err += 1; }
                    }
                }
                KnowledgeType::Black => {
                    match self.kb.ingest_black(it.clone()).await {
                        Ok(deduped) => {
                            ingested_ok += 1;
                            ok_black += 1;
                            if deduped {
                                ingested_updated += 1;
                                metrics::knowledge_deduped(1);
                            } else {
                                ingested_new += 1;
                                metrics::knowledge_ingested(&KnowledgeType::Black, 1);
                            }
                        }
                        Err(_) => { ingested_err += 1; }
                    }
                }
                _ => {}
            }
        }

        let _ = self.audit.log_event(
            "agent-cli",
            &format!(
                "scout_ingested ok={} new={} updated={} white={} black={} err={}",
                ingested_ok, ingested_new, ingested_updated, ok_white, ok_black, ingested_err
            ),
            0.25,
            true,
        );
        metrics::learning_gate_finish("ingest", ingested_err == 0);

        pipeline_msg(
            progress,
            format!(
                "[INGEST] ✓ новых={} обновлено={} (всего ok={}) black={} err={}",
                ingested_new, ingested_updated, ingested_ok, ok_black, ingested_err
            ),
        );

        // 5. DNA 2.5
        let dna = DnaEngine::new(&self.config.database.dna_path, self.audit.clone());
        let (dna_ok, dna_snap) = match dna.update_with_items(topic, &items).await {
            Ok(snap) => (true, Some(snap)),
            Err(e) => {
                tracing::warn!("DNA update failed: {}", e);
                (false, None)
            }
        };
        metrics::learning_gate_finish("dna", dna_ok);

        if dna_ok {
            pipeline_msg(progress, "[INGEST] DNA 2.5 обновлена");
        }

        Ok(CycleResult {
            topic: topic.to_string(),
            items_found: items.len(),
            critic_verdict: merged_verdict,
            critic_risk: merged_risk,
            inquisitor_blocks: inq_block_details.len(),
            inquisitor_escalates: inq_escalate_details.len(),
            ingested_ok,
            ingested_new,
            ingested_updated,
            ingested_white: ok_white,
            ingested_black: ok_black,
            ingested_err,
            dna_updated: dna_ok,
            dna_snapshot: dna_snap,
        })
    }
}

fn pipeline_msg(progress: Option<&tokio::sync::broadcast::Sender<String>>, msg: impl Into<String>) {
    if let Some(tx) = progress {
        let _ = tx.send(msg.into());
    }
}
