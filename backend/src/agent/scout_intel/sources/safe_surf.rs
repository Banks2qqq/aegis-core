//! НКЦКИ / safe-surf.ru — open RSS (новости и бюллетени).

use async_trait::async_trait;

use super::feed_parse::{extract_cves, parse_rss_items};
use super::http_util::{clip_chars, get_text};
use super::{ScoutFinding, ScoutSource, SourceMeta};

const DEFAULT_RSS_URL: &str = "https://safe-surf.ru/rss";
const DEFAULT_RSS_PATH: &str = "/opt/aegis/feeds/safe-surf-rss.xml";

async fn load_safe_surf_rss() -> Result<String, String> {
    if let Ok(p) = std::env::var("SAFE_SURF_RSS_PATH") {
        let path = p.trim();
        if !path.is_empty() && std::path::Path::new(path).is_file() {
            return std::fs::read_to_string(path).map_err(|e| format!("safe-surf file {path}: {e}"));
        }
    }
    if std::path::Path::new(DEFAULT_RSS_PATH).is_file() {
        return std::fs::read_to_string(DEFAULT_RSS_PATH)
            .map_err(|e| format!("safe-surf file {DEFAULT_RSS_PATH}: {e}"));
    }
    let url = std::env::var("SAFE_SURF_RSS_URL")
        .unwrap_or_else(|_| DEFAULT_RSS_URL.into());
    let u = url.trim();
    if u.is_empty() {
        return Err("пропуск: SAFE_SURF_RSS_URL пуст".into());
    }
    get_text(u, &[])
        .await
        .map_err(|e| format!("safe-surf RSS: {e}"))
}

fn severity_for_nkcki(blob: &str) -> &'static str {
    let lower = blob.to_lowercase();
    if lower.contains("критическ") || lower.contains("zero-day") || lower.contains("zero day") {
        return "critical";
    }
    if lower.contains("уязвимост")
        || lower.contains("cve-")
        || lower.contains("rce")
        || lower.contains("эксплуат")
    {
        return "high";
    }
    if lower.contains("статистик") || lower.contains("форум") || lower.contains("конференц") {
        return "low";
    }
    "medium"
}

pub struct SafeSurfNkckiSource;

#[async_trait]
impl ScoutSource for SafeSurfNkckiSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "safe_surf",
            label: "НКЦКИ / safe-surf.ru (RSS)",
            region: "RU",
            needs_api_key: false,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let xml = load_safe_surf_rss().await?;
        let items = parse_rss_items(&xml, limit.max(1));
        if items.is_empty() {
            return Err("пропуск: safe-surf RSS без элементов".into());
        }

        Ok(items
            .into_iter()
            .map(|item| {
                let blob = format!("{} {}", item.title, item.description);
                let cves = extract_cves(&blob);
                let severity = severity_for_nkcki(&blob);
                let mut tags = vec![
                    "safe-surf".into(),
                    "nkcki".into(),
                    "ru".into(),
                    "gossopka".into(),
                ];
                if blob.to_lowercase().contains("госсопка") {
                    tags.push("gossopka-event".into());
                }
                if !cves.is_empty() {
                    tags.push("vulnerability".into());
                }

                ScoutFinding {
                    id: uuid::Uuid::new_v4().to_string(),
                    source_id: "safe_surf".into(),
                    source_label: "НКЦКИ / safe-surf.ru".into(),
                    title: item.title.clone(),
                    severity: severity.into(),
                    summary: if item.description.is_empty() {
                        clip_chars(&item.title, 400)
                    } else {
                        clip_chars(&item.description, 500)
                    },
                    url: if item.link.is_empty() {
                        Some("https://safe-surf.ru".into())
                    } else {
                        Some(item.link.clone())
                    },
                    iocs: cves.clone(),
                    cves,
                    mitre_techniques: Vec::new(),
                    mitre_tactics: Vec::new(),
                    tags,
                    ..Default::default()
                }
            })
            .collect())
    }
}
