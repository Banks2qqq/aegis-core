//! Cisco Talos — IP reputation blocklist (open feed).
//!
//! Official Talos endpoints are often Cloudflare-protected from datacenter IPs.
//! Use `TALOS_BLOCKLIST_URL` or place a mirror at `TALOS_BLOCKLIST_PATH`
//! (default `/opt/aegis/feeds/talos-ip-blacklist.txt`). See `deploy/scout-sync-talos-feed.sh`.

use async_trait::async_trait;
use std::path::Path;

use super::feed_parse::parse_ip_blocklist;
use super::http_util::get_text;
use super::{ScoutFinding, ScoutSource, SourceMeta};

const DEFAULT_PATH: &str = "/opt/aegis/feeds/talos-ip-blacklist.txt";

const HTTP_CANDIDATES: &[&str] = &[
    "https://www.talosintelligence.com/documents/ip-blacklist",
    "https://talosintelligence.com/feeds/ip_filter.csv",
    "https://www.talosintelligence.com/feeds/current_reputation_blacklist",
];

async fn load_blocklist_text() -> Result<String, String> {
    if let Ok(url) = std::env::var("TALOS_BLOCKLIST_URL") {
        let u = url.trim();
        if !u.is_empty() {
            if u.starts_with("file://") {
                let path = u.trim_start_matches("file://");
                return std::fs::read_to_string(path)
                    .map_err(|e| format!("Talos file:// read: {e}"));
            }
            return get_text(u, &[]).await.map_err(|e| format!("Talos URL: {e}"));
        }
    }

    let path = std::env::var("TALOS_BLOCKLIST_PATH")
        .unwrap_or_else(|_| DEFAULT_PATH.into());
    if Path::new(&path).is_file() {
        return std::fs::read_to_string(&path).map_err(|e| format!("Talos path {path}: {e}"));
    }

    let mut last_err = String::new();
    for url in HTTP_CANDIDATES {
        match get_text(url, &[]).await {
            Ok(t) if parse_ip_blocklist(&t, 1).len() > 0 || t.contains('.') => return Ok(t),
            Ok(_) => last_err = format!("Talos {url}: empty blocklist"),
            Err(e) => last_err = e,
        }
    }

    Err(format!(
        "пропуск: Talos IP blacklist недоступен ({last_err}) — \
         задайте TALOS_BLOCKLIST_URL или файл {path} (deploy/scout-sync-talos-feed.sh)"
    ))
}

pub struct TalosBlocklistSource;

#[async_trait]
impl ScoutSource for TalosBlocklistSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "talos",
            label: "Cisco Talos (IP reputation)",
            region: "INTL",
            needs_api_key: false,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let text = load_blocklist_text().await?;
        let ips = parse_ip_blocklist(&text, limit.max(1));
        if ips.is_empty() {
            return Err("пропуск: Talos blocklist пуст".into());
        }

        let summary = format!(
            "Cisco Talos IP reputation blocklist: {} malicious/suspicious IPv4 (snapshot)",
            ips.len()
        );
        let mut structured = crate::scout_intel::structured::ScoutIocsStructured::default();
        structured.ips = ips.clone();

        Ok(vec![ScoutFinding {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: "talos".into(),
            source_label: "Cisco Talos".into(),
            title: format!("Talos IP blocklist — {} адресов", ips.len()),
            severity: "high".into(),
            summary: summary.clone(),
            url: Some("https://talosintelligence.com/reputation".into()),
            iocs: ips.clone(),
            cves: Vec::new(),
            mitre_techniques: Vec::new(),
            mitre_tactics: vec!["command-and-control".into()],
            tags: vec![
                "talos".into(),
                "blocklist".into(),
                "reputation".into(),
                "cisco".into(),
            ],
            structured,
            ..Default::default()
        }])
    }
}
