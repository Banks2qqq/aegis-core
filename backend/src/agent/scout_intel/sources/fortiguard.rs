//! Fortinet FortiGuard — open RSS (Outbreak Alerts + Threat Signal).

use async_trait::async_trait;

use super::feed_parse::{extract_cves, parse_rss_items};
use super::http_util::{clip_chars, get_text};
use super::{ScoutFinding, ScoutSource, SourceMeta};

const OUTBREAK_RSS: &str = "https://www.fortiguard.com/rss/outbreakalert.xml";
const DEFAULT_RSS_PATH: &str = "/opt/aegis/feeds/fortiguard-outbreak.xml";

async fn load_outbreak_rss() -> Result<String, String> {
    if let Ok(p) = std::env::var("FORTIGUARD_RSS_PATH") {
        let path = p.trim();
        if !path.is_empty() && std::path::Path::new(path).is_file() {
            return std::fs::read_to_string(path).map_err(|e| format!("FortiGuard file {path}: {e}"));
        }
    }
    if std::path::Path::new(DEFAULT_RSS_PATH).is_file() {
        return std::fs::read_to_string(DEFAULT_RSS_PATH)
            .map_err(|e| format!("FortiGuard file {DEFAULT_RSS_PATH}: {e}"));
    }
    if let Ok(url) = std::env::var("FORTIGUARD_RSS_URL") {
        let u = url.trim();
        if !u.is_empty() {
            return get_text(u, &[]).await.map_err(|e| format!("FortiGuard URL: {e}"));
        }
    }
    get_text(OUTBREAK_RSS, &[])
        .await
        .map_err(|e| format!("FortiGuard RSS: {e}"))
}

pub struct FortiGuardOutbreakSource;

#[async_trait]
impl ScoutSource for FortiGuardOutbreakSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "fortiguard",
            label: "FortiGuard Outbreak Alerts",
            region: "INTL",
            needs_api_key: false,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let xml = load_outbreak_rss().await?;
        let items = parse_rss_items(&xml, limit.max(1));
        if items.is_empty() {
            return Err("FortiGuard: RSS без элементов".into());
        }

        Ok(items
            .into_iter()
            .map(|item| {
                let blob = format!("{} {}", item.title, item.description);
                let cves = extract_cves(&blob);
                let severity = if cves.iter().any(|c| c.contains("2025") || c.contains("2026"))
                    || blob.to_lowercase().contains("critical")
                    || blob.to_lowercase().contains("zero-day")
                {
                    "critical"
                } else if blob.to_lowercase().contains("rce")
                    || blob.to_lowercase().contains("exploit")
                {
                    "high"
                } else {
                    "medium"
                };
                let mut tags = vec![
                    "fortiguard".into(),
                    "fortinet".into(),
                    "outbreak".into(),
                ];
                if blob.to_lowercase().contains("ransomware") {
                    tags.push("ransomware".into());
                }

                ScoutFinding {
                    id: uuid::Uuid::new_v4().to_string(),
                    source_id: "fortiguard".into(),
                    source_label: "FortiGuard".into(),
                    title: item.title.clone(),
                    severity: severity.into(),
                    summary: clip_chars(&item.description, 500),
                    url: if item.link.is_empty() {
                        None
                    } else {
                        Some(item.link.clone())
                    },
                    iocs: cves.clone(),
                    cves: cves.clone(),
                    mitre_techniques: Vec::new(),
                    mitre_tactics: Vec::new(),
                    tags,
                    ..Default::default()
                }
            })
            .collect())
    }
}
