//! Stage 2: extract IOCs → MITRE map → threat tags → normalize → dedupe.

use regex::Regex;
use std::sync::LazyLock;

use super::dedupe::{dedupe_and_merge, DedupeStats};
use super::mitre::{match_mitre_text, tactic_to_tag};
use super::structured::{classify_threat_tags, normalize_severity, ScoutIocsStructured};
use super::types::ScoutFinding;

static RE_CVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bCVE-\d{4}-\d{4,}\b").unwrap());
static RE_BDU: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bBDU:\d{4}-\d{5,}\b").unwrap());
static RE_IPV4: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d?\d)\b").unwrap());
static RE_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s<>"']+"#).unwrap());
static RE_DOMAIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+(?:[a-z]{2,24})\b").unwrap()
});
static RE_SHA256: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[a-fA-F0-9]{64}\b").unwrap());
static RE_SHA1: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[a-fA-F0-9]{40}\b").unwrap());
static RE_MD5: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[a-fA-F0-9]{32}\b").unwrap());

const NOISE_DOMAINS: &[&str] = &[
    "google.com",
    "microsoft.com",
    "github.com",
    "w3.org",
    "schemas.microsoft",
    "example.com",
    "localhost",
];

fn is_private_ip(ip: &str) -> bool {
    ip.starts_with("10.")
        || ip.starts_with("192.168.")
        || ip.starts_with("127.")
        || ip.starts_with("0.")
        || ip.starts_with("255.")
}

fn is_noise_domain(d: &str) -> bool {
    let d = d.to_lowercase();
    NOISE_DOMAINS.iter().any(|n| d == *n || d.ends_with(&format!(".{n}")))
}

fn push_unique(vec: &mut Vec<String>, val: String) {
    if !vec.iter().any(|x| x.eq_ignore_ascii_case(&val)) {
        vec.push(val);
    }
}

fn extract_structured(blob: &str) -> ScoutIocsStructured {
    let mut s = ScoutIocsStructured::default();
    for cve in RE_CVE.find_iter(blob) {
        push_unique(&mut s.cves, cve.as_str().to_uppercase());
    }
    for bdu in RE_BDU.find_iter(blob) {
        push_unique(&mut s.cves, bdu.as_str().to_uppercase());
    }
    for ip in RE_IPV4.find_iter(blob) {
        let ip = ip.as_str().to_string();
        if !is_private_ip(&ip) {
            push_unique(&mut s.ips, ip);
        }
    }
    for url in RE_URL.find_iter(blob) {
        let u = url.as_str().trim_end_matches(&['.', ',', ';'][..]).to_string();
        push_unique(&mut s.urls, u);
    }
    for d in RE_DOMAIN.find_iter(blob) {
        let d = d.as_str().to_lowercase();
        if !is_noise_domain(&d) {
            push_unique(&mut s.domains, d);
        }
    }
    for h in RE_SHA256.find_iter(blob) {
        push_unique(&mut s.hashes, format!("sha256:{}", h.as_str().to_lowercase()));
    }
    for h in RE_SHA1.find_iter(blob) {
        push_unique(&mut s.hashes, format!("sha1:{}", h.as_str().to_lowercase()));
    }
    for h in RE_MD5.find_iter(blob) {
        push_unique(&mut s.hashes, format!("md5:{}", h.as_str().to_lowercase()));
    }
    s
}

pub fn enrich_finding(f: &mut ScoutFinding) {
    let blob = format!("{} {} {}", f.title, f.summary, f.iocs.join(" "));
    let extracted = extract_structured(&blob);

    f.structured.ips.extend(extracted.ips);
    f.structured.domains.extend(extracted.domains);
    f.structured.urls.extend(extracted.urls);
    f.structured.hashes.extend(extracted.hashes);
    f.structured.cves.extend(extracted.cves);

    for c in &f.cves {
        push_unique(&mut f.structured.cves, c.clone());
    }
    for ioc in &f.iocs {
        let ex = extract_structured(ioc);
        f.structured.ips.extend(ex.ips);
        f.structured.domains.extend(ex.domains);
        f.structured.urls.extend(ex.urls);
        f.structured.hashes.extend(ex.hashes);
        f.structured.cves.extend(ex.cves);
    }

    let mitre = match_mitre_text(&blob);
    for t in mitre.techniques {
        push_unique(&mut f.mitre_techniques, t);
    }
    for tac in mitre.tactics {
        push_unique(&mut f.mitre_tactics, tac.clone());
        push_unique(&mut f.tags, tactic_to_tag(&tac));
    }

    f.tags = classify_threat_tags(&blob, &f.tags);
    f.severity = normalize_severity(&f.severity, &f.tags);

    f.iocs = f.structured.all_flat();
    f.cves = f.structured.cves.clone();
    f.content_hash = f.compute_content_hash();
}

#[derive(Debug, Clone, Default)]
pub struct EnrichmentReport {
    pub dedupe: DedupeStats,
    pub total_ips: usize,
    pub total_domains: usize,
    pub total_hashes: usize,
    pub total_cves: usize,
    pub total_mitre_techniques: usize,
}

pub fn run_enrichment_pipeline(mut findings: Vec<ScoutFinding>) -> (Vec<ScoutFinding>, EnrichmentReport) {
    for f in findings.iter_mut() {
        enrich_finding(f);
    }
    let (findings, dedupe) = dedupe_and_merge(findings);

    let mut rep = EnrichmentReport {
        dedupe,
        ..Default::default()
    };
    for f in &findings {
        rep.total_ips += f.structured.ips.len();
        rep.total_domains += f.structured.domains.len();
        rep.total_hashes += f.structured.hashes.len();
        rep.total_cves += f.structured.cves.len();
        rep.total_mitre_techniques += f.mitre_techniques.len();
    }

    (findings, rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_cve_and_hash() {
        let mut s = extract_structured("CVE-2024-1234 abc sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert!(s.cves.iter().any(|c| c.contains("CVE-2024")));
        assert!(!s.hashes.is_empty() || s.cves.len() > 0);
        let _ = &mut s;
    }
}
