//! Scout 2.0 — post-scout orchestration: autonomous reaction on critical threats (background heal).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::time::{sleep, Duration};

use crate::agent_registry::{AgentRegistry, HEALER_ID};
use crate::fstec_bdu::BduVulnerability;
use crate::healing_orchestrator::{HealingOrchestrator, PatchType};
use crate::learning_orchestrator::CycleResult;
use crate::scout_intel::ScoutFinding;

static HEALING_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Actionable threat surfaced by Scout (BDU or elevated cycle risk).
#[derive(Debug, Clone)]
pub struct ScoutCriticalThreat {
    pub threat_id: String,
    pub title: String,
    pub severity: String,
    pub source: String,
}

fn emit(tx: &tokio::sync::broadcast::Sender<String>, msg: impl Into<String>) {
    let _ = tx.send(msg.into());
}

/// Collect threats that warrant autonomous Self-Healing.
pub fn collect_critical_threats(
    vulns: &[BduVulnerability],
    cycle: &CycleResult,
) -> Vec<ScoutCriticalThreat> {
    let mut out: Vec<ScoutCriticalThreat> = vulns
        .iter()
        .filter(|v| v.severity == "critical")
        .take(5)
        .map(|v| ScoutCriticalThreat {
            threat_id: v.bdu_id.clone(),
            title: v.title.clone(),
            severity: v.severity.clone(),
            source: "fstec_bdu".into(),
        })
        .collect();

    // Elevated critic risk with high-severity BDU entries (cap total work).
    if cycle.critic_risk >= 0.85 {
        for v in vulns.iter().filter(|v| v.severity == "high").take(2) {
            if out.len() >= 5 {
                break;
            }
            if out.iter().any(|t| t.threat_id == v.bdu_id) {
                continue;
            }
            out.push(ScoutCriticalThreat {
                threat_id: v.bdu_id.clone(),
                title: v.title.clone(),
                severity: v.severity.clone(),
                source: "fstec_bdu_high_risk".into(),
            });
        }
    }

    out
}

/// Collect critical/high threats from unified Scout 2.0 findings (all sources).
pub fn collect_critical_threats_from_findings(
    findings: &[ScoutFinding],
    cycle: &CycleResult,
) -> Vec<ScoutCriticalThreat> {
    let mut out: Vec<ScoutCriticalThreat> = findings
        .iter()
        .filter(|f| f.severity == "critical")
        .take(5)
        .map(|f| ScoutCriticalThreat {
            threat_id: f
                .iocs
                .first()
                .cloned()
                .unwrap_or_else(|| f.id.clone()),
            title: f.title.clone(),
            severity: f.severity.clone(),
            source: f.source_id.clone(),
        })
        .collect();

    for f in findings.iter().filter(|f| f.severity == "high").take(3) {
        if out.len() >= 6 {
            break;
        }
        let tid = f
            .iocs
            .first()
            .cloned()
            .unwrap_or_else(|| f.id.clone());
        if out.iter().any(|t| t.threat_id == tid) {
            continue;
        }
        out.push(ScoutCriticalThreat {
            threat_id: tid,
            title: f.title.clone(),
            severity: f.severity.clone(),
            source: f.source_id.clone(),
        });
    }

    if cycle.critic_risk >= 0.9 && out.is_empty() {
        if let Some(f) = findings.first() {
            out.push(ScoutCriticalThreat {
                threat_id: f.id.clone(),
                title: f.title.clone(),
                severity: "high".into(),
                source: format!("{}_critic_escalation", f.source_id),
            });
        }
    }

    out
}

/// Schedule background healing; returns number of threats queued (0 if skipped/busy).
pub fn schedule_autonomous_healing(
    healing: Arc<HealingOrchestrator>,
    registry: Option<Arc<AgentRegistry>>,
    alert_tx: tokio::sync::broadcast::Sender<String>,
    threats: Vec<ScoutCriticalThreat>,
) -> usize {
    if threats.is_empty() {
        return 0;
    }

    if HEALING_IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        emit(
            &alert_tx,
            "[HEALING] — предыдущий автозапуск Scout ещё выполняется; пропуск дубликата",
        );
        return 0;
    }

    let n = threats.len();
    emit(
        &alert_tx,
        format!(
            "[SCOUT] Обнаружена критическая угроза → запуск Self-Healing ({} записей)",
            n
        ),
    );
    emit(
        &alert_tx,
        "[HEALING] Автозапуск по результатам Scout",
    );

    tokio::spawn(async move {
        run_healing_batch(healing, registry, alert_tx, threats).await;
        HEALING_IN_FLIGHT.store(false, Ordering::Release);
    });

    n
}

async fn run_healing_batch(
    healing: Arc<HealingOrchestrator>,
    registry: Option<Arc<AgentRegistry>>,
    alert_tx: tokio::sync::broadcast::Sender<String>,
    threats: Vec<ScoutCriticalThreat>,
) {
    if let Some(reg) = &registry {
        reg.mark_running(HEALER_ID, "Scout 2.0 autonomous healing (background)")
            .await;
    }

    let mut attempted = 0usize;
    let mut applied = 0usize;

    for (i, t) in threats.iter().enumerate() {
        if i > 0 {
            sleep(Duration::from_secs(2)).await;
        }
        attempted += 1;
        let label = t.threat_id.to_uppercase();
        emit(
            &alert_tx,
            format!(
                "[SCOUT] ● {} [{}] {} → Self-Healing",
                label,
                t.severity.to_uppercase(),
                clip_title(&t.title, 72)
            ),
        );

        let desc = format!("Scout 2.0 {} ({}): {}", t.source, t.severity, t.title);
        // Risk maps to patch type: Critical/High → HITL queue; Medium/Low → policy auto-apply.
        let patch_type = match t.severity.to_ascii_lowercase().as_str() {
            "critical" => PatchType::Custom,
            "high" => PatchType::Code,
            _ => PatchType::Config,
        };

        let heal_result = tokio::time::timeout(
            Duration::from_secs(90),
            healing.heal(&desc, patch_type),
        )
        .await;

        match heal_result {
            Ok(Ok(r)) => {
                emit(
                    &alert_tx,
                    format!(
                        "[HEALING] {} | applied={} verify={} sandbox={} risk={:?}",
                        label,
                        r.applied,
                        r.verification_passed,
                        r.sandbox_passed,
                        r.risk
                    ),
                );
                if r.applied {
                    applied += 1;
                } else if r.apply_mode == "pending_hitl" {
                    emit(
                        &alert_tx,
                        format!(
                            "[HEALING] {} → HITL очередь (patch_id={}) — ожидает approve",
                            label,
                            &r.patch_id[..r.patch_id.len().min(36)]
                        ),
                    );
                }
            }
            Ok(Err(e)) => {
                emit(
                    &alert_tx,
                    format!("[HEALING] ✗ {} — {}", label, clip_title(&e, 160)),
                );
            }
            Err(_) => {
                emit(
                    &alert_tx,
                    format!("[HEALING] ✗ {} — таймаут 90с", label),
                );
            }
        }
    }

    emit(
        &alert_tx,
        format!(
            "[HEALING] ✓ Автозапуск Scout завершён | attempted={} applied={}",
            attempted, applied
        ),
    );

    if let Some(reg) = &registry {
        reg.mark_success(
            HEALER_ID,
            &format!("Scout auto-heal — {}/{} applied", applied, attempted),
            Some(((applied * 33).min(100)) as u8),
        )
        .await;
    }
}

fn clip_title(s: &str, max: usize) -> String {
    let t: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        format!("{t}…")
    } else {
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fstec_bdu::BduVulnerability;

    fn vuln(id: &str, sev: &str) -> BduVulnerability {
        BduVulnerability {
            id: id.into(),
            bdu_id: id.into(),
            title: format!("title-{id}"),
            severity: sev.into(),
            url: "https://example".into(),
            published: None,
        }
    }

    #[test]
    fn collects_critical_only_by_default() {
        let vulns = vec![vuln("BDU-1", "high"), vuln("BDU-2", "critical")];
        let cycle = CycleResult {
            topic: "t".into(),
            items_found: 2,
            critic_verdict: "ok".into(),
            critic_risk: 0.5,
            inquisitor_blocks: 0,
            inquisitor_escalates: 0,
            ingested_ok: 2,
            ingested_new: 2,
            ingested_updated: 0,
            ingested_white: 0,
            ingested_black: 2,
            ingested_err: 0,
            dna_updated: false,
            dna_snapshot: None,
        };
        let t = collect_critical_threats(&vulns, &cycle);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].threat_id, "BDU-2");
    }
}
