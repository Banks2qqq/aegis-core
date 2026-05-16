//! Scout 2.0 immunity pipeline: multi-source OSINT → enrich → learn → fusion → heal → deception.

use std::sync::Arc;
use std::time::Instant;

use tokio::time::{timeout, Duration};

use crate::agent_registry::{AgentRegistry, SCOUT_ID};
use crate::fstec_bdu::BduVulnerability;
use crate::fusion_engine::FusionEngine;
use crate::honeypot_manager::HoneypotManager;
use crate::healing_orchestrator::HealingOrchestrator;
use crate::knowledge_item::KnowledgeItem;
use crate::learning_orchestrator::{CycleResult, LearningOrchestrator};
use crate::scout_intel::hub::{
    end_scout_run, run_intel_collection, try_begin_scout_run, SourceRunStatus,
};
use crate::scout_intel::ScoutFinding;
use crate::scout_orchestrator::{collect_critical_threats_from_findings, schedule_autonomous_healing};

pub struct PipelineOutcome {
    pub vulns: Vec<BduVulnerability>,
    pub findings: Vec<ScoutFinding>,
    pub cycle: CycleResult,
    pub fusion_updated: usize,
    pub deception_deployed: usize,
    pub healing_attempted: usize,
    pub healing_applied: usize,
    pub sources_ok: usize,
    pub sources_skipped: usize,
    pub sources_failed: usize,
    pub source_statuses: Vec<SourceRunStatus>,
    pub enrichment_merged: usize,
    pub total_iocs: usize,
    pub total_cves: usize,
}

/// Max wall-clock for the synchronous part of `/api/scout` (healing continues in background).
pub const SCOUT_PIPELINE_TIMEOUT_SECS: u64 = 180;

fn emit(tx: &tokio::sync::broadcast::Sender<String>, msg: impl Into<String>) {
    let _ = tx.send(msg.into());
}

/// Полный автономный цикл для `/api/scout` (все открытые источники из реестра).
pub async fn run_fstec_immunity_pipeline(
    learning: &LearningOrchestrator,
    fusion: Option<Arc<FusionEngine>>,
    honeypots: Option<Arc<HoneypotManager>>,
    healing: Option<Arc<HealingOrchestrator>>,
    registry: Option<Arc<AgentRegistry>>,
    alert_tx: tokio::sync::broadcast::Sender<String>,
) -> Result<PipelineOutcome, String> {
    try_begin_scout_run()?;
    let started = Instant::now();
    emit(
        &alert_tx,
        format!(
            "[SCOUT] Таймаут пайплайна: {}с (healing — в фоне после ответа API)",
            SCOUT_PIPELINE_TIMEOUT_SECS
        ),
    );

    let registry_for_timeout = registry.clone();
    let result = timeout(
        Duration::from_secs(SCOUT_PIPELINE_TIMEOUT_SECS),
        run_pipeline_inner(
            learning,
            fusion,
            honeypots,
            healing,
            registry,
            alert_tx.clone(),
        ),
    )
    .await;

    end_scout_run();

    let elapsed = started.elapsed().as_secs_f64();
    match result {
        Ok(Ok(outcome)) => {
            crate::metrics::record_scout_pipeline_run(
                "success",
                elapsed,
                outcome.findings.len(),
                outcome.healing_attempted,
            );
            Ok(outcome)
        }
        Ok(Err(e)) => {
            crate::metrics::record_scout_pipeline_run("error", elapsed, 0, 0);
            Err(e)
        }
        Err(_) => {
            crate::metrics::record_scout_pipeline_run("timeout", elapsed, 0, 0);
            emit(
                &alert_tx,
                format!(
                    "[SCOUT] ✗ Таймаут {}с — цикл прерван (проверьте LLM/источники)",
                    SCOUT_PIPELINE_TIMEOUT_SECS
                ),
            );
            if let Some(reg) = &registry_for_timeout {
                reg.mark_error(SCOUT_ID, "Scout pipeline timeout")
                    .await;
            }
            Err(format!(
                "Scout pipeline timeout ({}s)",
                SCOUT_PIPELINE_TIMEOUT_SECS
            ))
        }
    }
}

async fn run_pipeline_inner(
    learning: &LearningOrchestrator,
    fusion: Option<Arc<FusionEngine>>,
    honeypots: Option<Arc<HoneypotManager>>,
    healing: Option<Arc<HealingOrchestrator>>,
    registry: Option<Arc<AgentRegistry>>,
    alert_tx: tokio::sync::broadcast::Sender<String>,
) -> Result<PipelineOutcome, String> {
    if let Some(reg) = &registry {
        reg.mark_running(SCOUT_ID, "Scout 2.0 multi-source OSINT pipeline")
            .await;
    }

    let report = match run_intel_collection(&alert_tx, 25).await {
        r if r.findings.is_empty() && r.sources_ok == 0 => {
            emit(
                &alert_tx,
                "[SCOUT] ⚠ Ни один источник не вернул данные — проверьте ключи и сеть",
            );
            r
        }
        r => r,
    };
        let findings = report.findings.clone();

        let vulns: Vec<BduVulnerability> = findings
            .iter()
            .filter(|f| f.source_id == "fstec_bdu")
            .map(|f| {
                let bdu_id = f
                    .iocs
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "BDU-?".into());
                BduVulnerability {
                    id: bdu_id.clone(),
                    bdu_id,
                    title: f.title.clone(),
                    severity: f.severity.clone(),
                    url: f.url.clone().unwrap_or_else(|| "https://bdu.fstec.ru".into()),
                    published: None,
                }
            })
            .collect();

        let items: Vec<KnowledgeItem> = findings.iter().map(|f| f.to_knowledge_item()).collect();
        let topic = format!(
            "Scout 2.0 OSINT: {} findings from {} sources",
            findings.len(),
            report.sources_ok
        );

        emit(&alert_tx, "[CRITIC] ▶ Оценка риска собранных данных");
        let cycle = learning
            .run_cycle_from_items(&topic, items.clone(), Some(&alert_tx))
            .await?;
        crate::metrics::record_scout_run(&items);

        let threat_level = cycle.critic_risk.clamp(0.0, 1.0);
        let mut fusion_updated = 0usize;

        if let Some(f) = fusion {
            emit(
                &alert_tx,
                "[FUSION] ▶ Корреляция IOC/CVE по всем источникам Scout",
            );
            for finding in &findings {
                let severity = match finding.severity.as_str() {
                    "critical" => 0.92,
                    "high" => 0.78,
                    "medium" => 0.6,
                    _ => 0.45,
                };
                let label = format!("{} | {}", finding.source_id, finding.title);
                if f
                    .ingest(
                        &finding.source_id,
                        &label,
                        severity,
                        Some(&finding.id),
                        Some("scout_id"),
                    )
                    .await
                    .is_some()
                {
                    fusion_updated += 1;
                }
            }
            emit(
                &alert_tx,
                format!("[FUSION] ✓ Кластеров обновлено/создано: {}", fusion_updated),
            );
        } else {
            emit(&alert_tx, "[FUSION] — недоступен");
        }

        let healing_applied = 0usize;
        let mut healing_attempted = 0usize;
        let critical_threats = collect_critical_threats_from_findings(&findings, &cycle);
        if let Some(heal) = healing.clone() {
            if critical_threats.is_empty() {
                emit(
                    &alert_tx,
                    "[HEALING] — критических угроз нет; автономный heal не требуется",
                );
            } else {
                emit(
                    &alert_tx,
                    format!(
                        "[WAR ROOM] Автореакция: critic={:.2} | critical/high={} → Self-Healing (фон)",
                        cycle.critic_risk,
                        critical_threats.len()
                    ),
                );
                for t in critical_threats.iter().take(5) {
                    emit(
                        &alert_tx,
                        format!(
                            "[WAR ROOM]   → [{}] {} | {}",
                            t.severity.to_uppercase(),
                            t.source,
                            clip(&t.title, 70)
                        ),
                    );
                }
                healing_attempted = schedule_autonomous_healing(
                    heal,
                    registry.clone(),
                    alert_tx.clone(),
                    critical_threats,
                );
            }
        } else {
            emit(&alert_tx, "[HEALING] — HealingOrchestrator не инициализирован");
        }

        let mut deception_deployed = 0usize;
        if let Some(hp) = honeypots {
            emit(
                &alert_tx,
                format!(
                    "[DECEPTION] ▶ Honeypots (threat_level={:.2})",
                    threat_level
                ),
            );
            match hp.auto_deploy_honeypots(threat_level).await {
                Ok(ids) => {
                    deception_deployed = ids.len();
                    emit(
                        &alert_tx,
                        format!("[DECEPTION] ✓ Ловушек: {}", deception_deployed),
                    );
                }
                Err(e) => {
                    emit(
                        &alert_tx,
                        format!("[DECEPTION] ✗ Ошибка развёртывания: {}", clip(&e, 120)),
                    );
                    tracing::warn!(error = %e, "scout deception deploy failed");
                }
            }
        }

        emit(
            &alert_tx,
            format!(
                "[WAR ROOM] ✓ Цикл завершён | findings={} | sources OK={} skip={} err={} | ingest +{}/~{} | fusion={} | heal_queued={} | IOC={} CVE={}",
                findings.len(),
                report.sources_ok,
                report.sources_skipped,
                report.sources_failed,
                cycle.ingested_new,
                cycle.ingested_updated,
                fusion_updated,
                healing_attempted,
                report.total_iocs,
                report.total_cves
            ),
        );

        tracing::info!(
            findings = findings.len(),
            sources_ok = report.sources_ok,
            sources_skipped = report.sources_skipped,
            sources_failed = report.sources_failed,
            critic = %cycle.critic_verdict,
            critic_risk = cycle.critic_risk,
            heal_scheduled = healing_attempted,
            "scout pipeline complete"
        );

        if let Some(reg) = &registry {
            reg.mark_success(
                SCOUT_ID,
                &format!(
                    "Scout 2.0 — {} findings, {} sources, heal_scheduled={}",
                    findings.len(),
                    report.sources_ok,
                    healing_attempted
                ),
                Some((findings.len().min(100)) as u8),
            )
            .await;
        }

        Ok(PipelineOutcome {
            vulns,
            findings,
            cycle,
            fusion_updated,
            deception_deployed,
            healing_attempted,
            healing_applied,
            sources_ok: report.sources_ok,
            sources_skipped: report.sources_skipped,
            sources_failed: report.sources_failed,
            source_statuses: report.source_statuses,
            enrichment_merged: report.enrichment_merged,
            total_iocs: report.total_iocs,
            total_cves: report.total_cves,
        })
}

fn clip(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}
