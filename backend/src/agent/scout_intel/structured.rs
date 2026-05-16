//! Structured IOC buckets and threat classification.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoutIocsStructured {
    pub ips: Vec<String>,
    pub domains: Vec<String>,
    pub urls: Vec<String>,
    /// `sha256:…`, `md5:…`, `sha1:…`
    pub hashes: Vec<String>,
    pub cves: Vec<String>,
}

impl ScoutIocsStructured {
    pub fn all_flat(&self) -> Vec<String> {
        let mut v = Vec::new();
        v.extend(self.ips.clone());
        v.extend(self.domains.clone());
        v.extend(self.urls.clone());
        v.extend(
            self.hashes
                .iter()
                .map(|h| h.split_once(':').map(|(_, v)| v).unwrap_or(h).to_string()),
        );
        v.extend(self.cves.clone());
        v
    }

    pub fn primary_key(&self) -> Option<String> {
        self.hashes
            .first()
            .cloned()
            .or_else(|| self.cves.first().cloned())
            .or_else(|| self.ips.first().cloned())
            .or_else(|| self.domains.first().cloned())
            .or_else(|| self.urls.first().cloned())
    }
}

/// High-level threat tags for operators (MITRE-aligned).
pub fn classify_threat_tags(blob: &str, existing: &[String]) -> Vec<String> {
    let lower = blob.to_lowercase();
    let mut tags: Vec<String> = existing.to_vec();

    let rules: &[(&str, &[&str])] = &[
        ("ransomware", &["ransomware", "вымогател", "encrypt", "шифров"]),
        ("apt", &["apt", "advanced persistent", "state-sponsored"]),
        ("initial-access", &["phishing", "exploit", "initial access", "вектор"]),
        ("credential-access", &["credential", "password", "brute", "dump", "lsass"]),
        ("command-and-control", &["c2", "beacon", "command and control"]),
        ("lateral-movement", &["lateral", "rdp", "smb spread"]),
        ("exfiltration", &["exfil", "exfiltration", "утечк"]),
        ("malware", &["trojan", "backdoor", "loader", "rat", "botnet"]),
        ("vulnerability", &["cve-", "уязвимост", "bdu-"]),
    ];

    for (tag, kws) in rules {
        if kws.iter().any(|k| lower.contains(k)) && !tags.iter().any(|t| t == tag) {
            tags.push(tag.to_string());
        }
    }
    tags
}

pub fn normalize_severity(raw: &str, tags: &[String]) -> String {
    let s = raw.to_lowercase();
    if tags.iter().any(|t| t == "ransomware") || s == "critical" {
        return "critical".into();
    }
    if s == "high" || s == "critical" {
        return s;
    }
    if tags.iter().any(|t| t == "apt" || t == "vulnerability") {
        return "high".into();
    }
    if s.is_empty() {
        return "medium".into();
    }
    s
}
