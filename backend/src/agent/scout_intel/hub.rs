//! Parallel collection from all registered open sources.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::broadcast;
use tokio::time::{timeout, Duration};

use super::enrichment::run_enrichment_pipeline;
use super::sources::all_sources;
use super::types::ScoutFinding;
use super::ux;

static SCOUT_RUN_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
/// Unix secs when current SCOUT run started (0 = idle).
static SCOUT_RUN_STARTED_AT: AtomicU64 = AtomicU64::new(0);

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn scout_lock_max_secs() -> u64 {
    std::env::var("SCOUT_LOCK_MAX_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceRunStatus {
    pub id: String,
    pub label: String,
    pub status: String,
    pub count: usize,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct ScoutCollectionReport {
    pub findings: Vec<ScoutFinding>,
    pub source_statuses: Vec<SourceRunStatus>,
    pub sources_ok: usize,
    pub sources_skipped: usize,
    pub sources_failed: usize,
    pub total_raw: usize,
    pub total_iocs: usize,
    pub total_cves: usize,
    pub total_mitre_tags: usize,
    pub enrichment_merged: usize,
    pub total_ips: usize,
    pub total_domains: usize,
    pub total_hashes: usize,
}

pub fn try_begin_scout_run() -> Result<(), String> {
    if SCOUT_RUN_IN_FLIGHT.load(Ordering::Acquire) {
        let started = SCOUT_RUN_STARTED_AT.load(Ordering::Acquire);
        let max_secs = scout_lock_max_secs();
        if started > 0 && now_unix().saturating_sub(started) > max_secs {
            tracing::warn!(
                started,
                max_secs,
                "SCOUT lock stale — force release (phase 4)"
            );
            end_scout_run();
        } else {
            return Err("SCOUT уже выполняется — дождитесь завершения предыдущего цикла".into());
        }
    }
    if SCOUT_RUN_IN_FLIGHT
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("SCOUT уже выполняется — дождитесь завершения предыдущего цикла".into());
    }
    SCOUT_RUN_STARTED_AT.store(now_unix(), Ordering::Release);
    Ok(())
}

pub fn end_scout_run() {
    SCOUT_RUN_IN_FLIGHT.store(false, Ordering::Release);
    SCOUT_RUN_STARTED_AT.store(0, Ordering::Release);
}

pub async fn run_intel_collection(
    alert_tx: &broadcast::Sender<String>,
    per_source_limit: usize,
) -> ScoutCollectionReport {
    let sources = all_sources();
    ux::emit_run_start(alert_tx, sources.len());

    let mut handles = Vec::new();
    for src in sources {
        handles.push(tokio::spawn(async move {
            let meta = src.meta();
            let label = meta.label.to_string();
            let id = meta.id.to_string();
            let result = timeout(Duration::from_secs(35), src.collect(per_source_limit)).await;
            (id, label, meta.needs_api_key, result)
        }));
    }

    let mut all_findings = Vec::new();
    let mut statuses = Vec::new();
    let mut ok = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut total_raw = 0usize;

    for h in handles {
        match h.await {
            Ok((id, label, needs_key, inner)) => match inner {
                Ok(Ok(findings)) => {
                    let n = findings.len();
                    total_raw += n;
                    tracing::info!(source = %id, count = n, "scout_intel source ok");
                    let st = SourceRunStatus {
                        id: id.clone(),
                        label: label.clone(),
                        status: "ok".into(),
                        count: n,
                        note: "данные получены".into(),
                    };
                    ux::emit_source_result(alert_tx, &st);
                    crate::metrics::record_scout_intel_source(&id, "ok");
                    all_findings.extend(findings);
                    ok += 1;
                    statuses.push(st);
                }
                Ok(Err(e)) => {
                    let is_skip = needs_key
                        || e.contains("пропуск")
                        || e.contains("ABUSECH_API_KEY")
                        || e.contains("Unauthorized");
                    if is_skip {
                        tracing::info!(source = %id, note = %e, "scout_intel source skipped");
                    } else {
                        tracing::warn!(source = %id, error = %e, "scout_intel source error");
                    }
                    let st = SourceRunStatus {
                        id: id.clone(),
                        label: label.clone(),
                        status: if is_skip { "skipped" } else { "error" }.into(),
                        count: 0,
                        note: e.clone(),
                    };
                    ux::emit_source_result(alert_tx, &st);
                    crate::metrics::record_scout_intel_source(
                        &id,
                        if is_skip { "skipped" } else { "error" },
                    );
                    if is_skip {
                        skipped += 1;
                    } else {
                        failed += 1;
                    }
                    statuses.push(st);
                }
                Err(_) => {
                    crate::metrics::record_scout_intel_source(&id, "timeout");
                    tracing::warn!(source = %id, "scout_intel source timeout");
                    let st = SourceRunStatus {
                        id,
                        label,
                        status: "error".into(),
                        count: 0,
                        note: "таймаут 35с".into(),
                    };
                    ux::emit_source_result(alert_tx, &st);
                    failed += 1;
                    statuses.push(st);
                }
            },
            Err(_) => {}
        }
    }

    emit(
        alert_tx,
        "[SCOUT] ▶ Обогащение: CVE / IOC / MITRE ATT&CK / дедуп / теги риска",
    );
    let (findings, enrich_rep) = run_enrichment_pipeline(all_findings);

    let total_iocs: usize = findings.iter().map(|f| f.iocs.len()).sum();
    let total_cves: usize = findings.iter().map(|f| f.cves.len()).sum();
    let total_mitre_tags: usize = findings
        .iter()
        .map(|f| f.mitre_techniques.len() + f.tags.len())
        .sum();

    let report = ScoutCollectionReport {
        findings: findings.clone(),
        source_statuses: statuses,
        sources_ok: ok,
        sources_skipped: skipped,
        sources_failed: failed,
        total_raw,
        total_iocs,
        total_cves,
        total_mitre_tags,
        enrichment_merged: enrich_rep.dedupe.merged_count,
        total_ips: enrich_rep.total_ips,
        total_domains: enrich_rep.total_domains,
        total_hashes: enrich_rep.total_hashes,
    };

    ux::emit_enrichment_summary(alert_tx, &enrich_rep, &report);
    ux::emit_top_findings(alert_tx, &report, 10);
    ux::emit_executive_summary(alert_tx, &report);
    report
}

fn emit(tx: &broadcast::Sender<String>, msg: impl Into<String>) {
    let _ = tx.send(msg.into());
}
