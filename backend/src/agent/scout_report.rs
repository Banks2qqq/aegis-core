//! Structured Scout 2.0 operator report (honest autonomy + enrichment visibility).

use serde::Serialize;

use crate::patch_applier::heal_apply_enforced;
use crate::scout_intel::ScoutFinding;
use crate::scout_orchestrator::ScoutCriticalThreat;
use crate::scout_pipeline::PipelineOutcome;

#[derive(Debug, Clone, Serialize)]
pub struct ScoutFindingView {
    pub id: String,
    pub source_id: String,
    pub source_label: String,
    pub title: String,
    pub severity: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub cves: Vec<String>,
    pub mitre_techniques: Vec<String>,
    pub tags: Vec<String>,
    pub ioc_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoutReactionView {
    pub threat_id: String,
    pub title: String,
    pub severity: String,
    pub source: String,
    /// `hitl_queue` | `auto_apply_eligible` | `scheduled_background`
    pub disposition: String,
    pub policy_note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoutAutonomyPolicy {
    pub heal_apply_enforced: bool,
    pub description_ru: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoutEnrichmentView {
    pub merged_duplicates: usize,
    pub total_iocs: usize,
    pub total_cves: usize,
    pub total_ips: usize,
    pub total_domains: usize,
    pub total_hashes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoutOperatorReport {
    pub executive_summary_ru: String,
    pub top_findings: Vec<ScoutFindingView>,
    pub reactions: Vec<ScoutReactionView>,
    pub enrichment: ScoutEnrichmentView,
    pub autonomy: ScoutAutonomyPolicy,
}

fn severity_rank(s: &str) -> u8 {
    match s.to_ascii_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn finding_view(f: &ScoutFinding) -> ScoutFindingView {
    ScoutFindingView {
        id: f.id.clone(),
        source_id: f.source_id.clone(),
        source_label: f.source_label.clone(),
        title: f.title.clone(),
        severity: f.severity.clone(),
        summary: clip(&f.summary, 280),
        url: f.url.clone(),
        cves: f.cves.clone(),
        mitre_techniques: f.mitre_techniques.clone(),
        tags: f.tags.clone(),
        ioc_count: f.structured.all_flat().len(),
    }
}

pub fn top_findings(findings: &[ScoutFinding], limit: usize) -> Vec<ScoutFindingView> {
    let mut sorted: Vec<&ScoutFinding> = findings.iter().collect();
    sorted.sort_by(|a, b| {
        severity_rank(&b.severity)
            .cmp(&severity_rank(&a.severity))
            .then_with(|| a.title.cmp(&b.title))
    });
    sorted
        .into_iter()
        .take(limit)
        .map(finding_view)
        .collect()
}

pub fn build_reactions(threats: &[ScoutCriticalThreat]) -> Vec<ScoutReactionView> {
    let apply = heal_apply_enforced();
    threats
        .iter()
        .map(|t| {
            let sev = t.severity.to_ascii_lowercase();
            let (disposition, policy_note) = match sev.as_str() {
                "critical" | "high" => {
                    if apply {
                        (
                            "auto_apply_eligible",
                            "После Docker-sandbox возможно автоприменение; Critical/High могут остаться в HITL по риску",
                        )
                    } else {
                        (
                            "hitl_queue",
                            "Пилот: AEGIS_HEAL_APPLY=0 — патч в очередь /dashboard/healing после sandbox",
                        )
                    }
                }
                _ => (
                    "auto_apply_eligible",
                    "Medium/Low: после sandbox политика допускает автоприменение",
                ),
            };
            ScoutReactionView {
                threat_id: t.threat_id.clone(),
                title: clip(&t.title, 160),
                severity: t.severity.clone(),
                source: t.source.clone(),
                disposition: disposition.into(),
                policy_note: policy_note.into(),
            }
        })
        .collect()
}

fn count_structured(findings: &[ScoutFinding]) -> (usize, usize, usize) {
    let mut ips = 0usize;
    let mut domains = 0usize;
    let mut hashes = 0usize;
    for f in findings {
        ips += f.structured.ips.len();
        domains += f.structured.domains.len();
        hashes += f.structured.hashes.len();
    }
    (ips, domains, hashes)
}

pub fn autonomy_policy() -> ScoutAutonomyPolicy {
    let heal_apply_enforced = heal_apply_enforced();
    let description_ru = if heal_apply_enforced {
        "HEAL_APPLY=1: после sandbox патчи могут применяться автоматически (Critical/High — с HITL при высоком риске)."
            .into()
    } else {
        "HEAL_APPLY=0 (пилот primary): автореакция = sandbox + очередь HITL; оператор подтверждает в /dashboard/healing."
            .into()
    };
    ScoutAutonomyPolicy {
        heal_apply_enforced,
        description_ru,
    }
}

pub fn build_operator_report(
    outcome: &PipelineOutcome,
    scheduled_threats: &[ScoutCriticalThreat],
) -> ScoutOperatorReport {
    let (ips, domains, hashes) = count_structured(&outcome.findings);
    let reactions = build_reactions(scheduled_threats);
    let autonomy = autonomy_policy();

    let executive_summary_ru = format!(
        "Scout 2.0: {} уникальных находок · источников OK {} (пропущено {}, ошибок {}). Critic: {} (risk {:.2}). \
Автореакция: {} угроз в фоне (heal_scheduled={}). Fusion +{}, honeypots +{}. Обогащение: IOC {} · CVE {} · дедуп {}.",
        outcome.findings.len(),
        outcome.sources_ok,
        outcome.sources_skipped,
        outcome.sources_failed,
        outcome.cycle.critic_verdict,
        outcome.cycle.critic_risk,
        reactions.len(),
        outcome.healing_attempted,
        outcome.fusion_updated,
        outcome.deception_deployed,
        outcome.total_iocs,
        outcome.total_cves,
        outcome.enrichment_merged,
    );

    ScoutOperatorReport {
        executive_summary_ru,
        top_findings: top_findings(&outcome.findings, 12),
        reactions,
        enrichment: ScoutEnrichmentView {
            merged_duplicates: outcome.enrichment_merged,
            total_iocs: outcome.total_iocs,
            total_cves: outcome.total_cves,
            total_ips: ips,
            total_domains: domains,
            total_hashes: hashes,
        },
        autonomy,
    }
}

fn clip(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}
