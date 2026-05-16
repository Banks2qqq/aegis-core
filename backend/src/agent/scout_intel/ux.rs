//! Human-friendly War Room messages for operators.

use tokio::sync::broadcast;

use super::enrichment::EnrichmentReport;
use super::hub::{ScoutCollectionReport, SourceRunStatus};

pub fn emit(tx: &broadcast::Sender<String>, msg: impl Into<String>) {
    let _ = tx.send(msg.into());
}

pub fn emit_run_start(tx: &broadcast::Sender<String>, source_count: usize) {
    emit(
        tx,
        format!(
            "[SCOUT] ▶ Старт разведки: {} открытых источников (автономный цикл)",
            source_count
        ),
    );
    emit(
        tx,
        "[SCOUT] Цепочка: сбор → обогащение (IOC/CVE/MITRE) → Critic → Ingest → Fusion → Healing → Deception",
    );
}

pub fn emit_source_result(tx: &broadcast::Sender<String>, s: &SourceRunStatus) {
    let icon = match s.status.as_str() {
        "ok" => "✓",
        "skipped" => "○",
        _ => "✗",
    };
    emit(
        tx,
        format!(
            "[SCOUT] {} {} — {} записей ({})",
            icon, s.label, s.count, s.note
        ),
    );
}

pub fn emit_top_findings(tx: &broadcast::Sender<String>, report: &ScoutCollectionReport, limit: usize) {
    let critical: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.severity == "critical" || f.severity == "high")
        .take(limit)
        .collect();
    if critical.is_empty() {
        emit(tx, "[SCOUT] — нет critical/high в сводке (см. ingest)");
        return;
    }
    emit(tx, "[SCOUT] ━━━ Важное для оператора ━━━");
    for f in critical {
        let mitre = if f.mitre_techniques.is_empty() {
            String::new()
        } else {
            format!(" | MITRE {}", f.mitre_techniques.join(","))
        };
        let cve = if f.cves.is_empty() {
            String::new()
        } else {
            format!(" | {}", f.cves.join(","))
        };
        let tags = if f.tags.is_empty() {
            String::new()
        } else {
            format!(" | tags: {}", f.tags.join(","))
        };
        emit(
            tx,
            format!(
                "[SCOUT] ● [{}] {} — {}{}{}{}",
                f.severity.to_uppercase(),
                f.source_label,
                clip(&f.title, 60),
                cve,
                mitre,
                tags
            ),
        );
        if !f.structured.hashes.is_empty() {
            emit(
                tx,
                format!(
                    "[SCOUT]   hash: {}",
                    f.structured.hashes.iter().take(2).cloned().collect::<Vec<_>>().join(", ")
                ),
            );
        }
        if let Some(u) = &f.url {
            emit(tx, format!("[SCOUT]   ↳ {}", u));
        }
    }
}

pub fn emit_enrichment_summary(
    tx: &broadcast::Sender<String>,
    enrich: &EnrichmentReport,
    report: &ScoutCollectionReport,
) {
    emit(tx, "[SCOUT] ━━━ Обогащение (этап 2) ━━━");
    emit(
        tx,
        format!(
            "[SCOUT] Дедуп: {} → {} уникальных (объединено дублей: {})",
            enrich.dedupe.input_count,
            enrich.dedupe.output_count,
            enrich.dedupe.merged_count
        ),
    );
    emit(
        tx,
        format!(
            "[SCOUT] Структура IOC: IP={} | домены={} | хэши={} | CVE={} | MITRE tech={}",
            report.total_ips,
            report.total_domains,
            report.total_hashes,
            report.total_cves,
            enrich.total_mitre_techniques
        ),
    );
}

pub fn emit_executive_summary(tx: &broadcast::Sender<String>, report: &ScoutCollectionReport) {
    emit(tx, "[SCOUT] ━━━ Сводка для человека ━━━");
    emit(
        tx,
        format!(
            "[SCOUT] Источников: {} OK / {} пропущено / {} ошибок | Всего записей: {} (уникальных: {})",
            report.sources_ok,
            report.sources_skipped,
            report.sources_failed,
            report.total_raw,
            report.findings.len()
        ),
    );
    emit(
        tx,
        format!(
            "[SCOUT] IOC: {} | CVE: {} | MITRE-теги: {}",
            report.total_iocs, report.total_cves, report.total_mitre_tags
        ),
    );
}

fn clip(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}
