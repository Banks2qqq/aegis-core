//! Cross-source deduplication with IOC/CVE/hash merge.

use std::collections::HashMap;

use super::types::ScoutFinding;

#[derive(Debug, Clone, Default)]
pub struct DedupeStats {
    pub input_count: usize,
    pub output_count: usize,
    pub merged_count: usize,
}

pub fn dedupe_and_merge(findings: Vec<ScoutFinding>) -> (Vec<ScoutFinding>, DedupeStats) {
    let input_count = findings.len();
    let mut buckets: HashMap<String, ScoutFinding> = HashMap::new();
    let mut merged_count = 0usize;

    for f in findings {
        let key = dedupe_key(&f);
        if let Some(existing) = buckets.get_mut(&key) {
            merge_into(existing, &f);
            merged_count += 1;
        } else {
            buckets.insert(key, f);
        }
    }

    let mut out: Vec<ScoutFinding> = buckets.into_values().collect();
    out.sort_by(|a, b| severity_rank(&b.severity).cmp(&severity_rank(&a.severity)));
    let output_count = out.len();
    (
        out,
        DedupeStats {
            input_count,
            output_count,
            merged_count,
        },
    )
}

fn dedupe_key(f: &ScoutFinding) -> String {
    if let Some(h) = f.structured.hashes.first() {
        return format!("hash:{}", h.to_lowercase());
    }
    if let Some(c) = f.cves.first() {
        return format!("cve:{}", c.to_uppercase());
    }
    if let Some(ip) = f.structured.ips.first() {
        return format!("ip:{}", ip);
    }
    if let Some(d) = f.structured.domains.first() {
        return format!("domain:{}", d.to_lowercase());
    }
    format!(
        "title:{}|{}",
        f.source_id,
        f.title.to_lowercase().chars().take(100).collect::<String>()
    )
}

fn merge_into(dst: &mut ScoutFinding, src: &ScoutFinding) {
    merge_vec(&mut dst.iocs, &src.iocs);
    merge_vec(&mut dst.cves, &src.cves);
    merge_vec(&mut dst.tags, &src.tags);
    merge_vec(&mut dst.mitre_techniques, &src.mitre_techniques);
    merge_vec(&mut dst.mitre_tactics, &src.mitre_tactics);
    merge_structured(&mut dst.structured, &src.structured);
    if severity_rank(&src.severity) > severity_rank(&dst.severity) {
        dst.severity = src.severity.clone();
    }
    if !src.summary.is_empty() && !dst.summary.contains(&src.summary) {
        dst.summary = format!("{} | {}", dst.summary, src.summary);
    }
    if dst.url.is_none() {
        dst.url = src.url.clone();
    }
    if !src.source_label.contains(&dst.source_label) {
        dst.source_label = format!("{}, {}", dst.source_label, src.source_label);
    }
}

fn merge_structured(dst: &mut super::structured::ScoutIocsStructured, src: &super::structured::ScoutIocsStructured) {
    merge_vec(&mut dst.ips, &src.ips);
    merge_vec(&mut dst.domains, &src.domains);
    merge_vec(&mut dst.urls, &src.urls);
    merge_vec(&mut dst.hashes, &src.hashes);
    merge_vec(&mut dst.cves, &src.cves);
}

fn merge_vec(dst: &mut Vec<String>, src: &[String]) {
    for s in src {
        if !dst.iter().any(|d| d.eq_ignore_ascii_case(s)) {
            dst.push(s.clone());
        }
    }
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}
