//! RU / regional security blogs via RSS (phase 4) — URL or mirrored file on VPS.

use async_trait::async_trait;

use super::feed_parse::{extract_cves, parse_rss_items};
use super::http_util::{clip_chars, get_text};
use super::{ScoutFinding, ScoutSource, SourceMeta};

#[derive(Clone, Copy)]
pub struct BlogRssConfig {
    pub id: &'static str,
    pub label: &'static str,
    pub region: &'static str,
    pub default_url: &'static str,
    pub default_path: &'static str,
    pub env_url: &'static str,
    pub env_path: &'static str,
    pub extra_tags: &'static [&'static str],
}

pub struct BlogRssSource {
    cfg: BlogRssConfig,
}

impl BlogRssSource {
    pub const fn new(cfg: BlogRssConfig) -> Self {
        Self { cfg }
    }

    async fn load_xml(&self) -> Result<String, String> {
        if let Ok(p) = std::env::var(self.cfg.env_path) {
            let path = p.trim();
            if !path.is_empty() && std::path::Path::new(path).is_file() {
                return std::fs::read_to_string(path)
                    .map_err(|e| format!("{} file {path}: {e}", self.cfg.id));
            }
        }
        if std::path::Path::new(self.cfg.default_path).is_file() {
            return std::fs::read_to_string(self.cfg.default_path).map_err(|e| {
                format!("{} file {}: {e}", self.cfg.id, self.cfg.default_path)
            });
        }
        let url = std::env::var(self.cfg.env_url)
            .unwrap_or_else(|_| self.cfg.default_url.into());
        let u = url.trim();
        if u.is_empty() {
            return Err(format!(
                "пропуск: {} — задайте {} или положите feed в {}",
                self.cfg.label, self.cfg.env_url, self.cfg.default_path
            ));
        }
        get_text(u, &[])
            .await
            .map_err(|e| format!("{} RSS: {e}", self.cfg.label))
    }
}

fn severity_from_blob(blob: &str) -> &'static str {
    let lower = blob.to_lowercase();
    if lower.contains("критическ")
        || lower.contains("zero-day")
        || lower.contains("zero day")
        || lower.contains("apt")
    {
        return "critical";
    }
    if lower.contains("уязвимост")
        || lower.contains("cve-")
        || lower.contains("rce")
        || lower.contains("взлом")
        || lower.contains("утечк")
    {
        return "high";
    }
    if lower.contains("статистик") || lower.contains("мероприят") {
        return "low";
    }
    "medium"
}

#[async_trait]
impl ScoutSource for BlogRssSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: self.cfg.id,
            label: self.cfg.label,
            region: self.cfg.region,
            needs_api_key: false,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let xml = self.load_xml().await?;
        if !xml.contains("<item") && !xml.contains(":item") {
            return Err(format!(
                "пропуск: {} — ответ не похож на RSS (используйте mirror в {}",
                self.cfg.label, self.cfg.default_path
            ));
        }
        let items = parse_rss_items(&xml, limit.max(1));
        if items.is_empty() {
            return Err(format!("пропуск: {} — RSS без элементов", self.cfg.label));
        }

        Ok(items
            .into_iter()
            .map(|item| {
                let blob = format!("{} {}", item.title, item.description);
                let cves = extract_cves(&blob);
                let severity = severity_from_blob(&blob);
                let mut tags = vec![
                    "scout_2".into(),
                    "blog_rss".into(),
                    self.cfg.id.to_string(),
                    "ru".into(),
                ];
                tags.extend(self.cfg.extra_tags.iter().map(|t| (*t).to_string()));
                if !cves.is_empty() {
                    tags.push("vulnerability".into());
                }

                ScoutFinding {
                    id: uuid::Uuid::new_v4().to_string(),
                    source_id: self.cfg.id.to_string(),
                    source_label: self.cfg.label.to_string(),
                    title: item.title.clone(),
                    severity: severity.into(),
                    summary: if item.description.is_empty() {
                        clip_chars(&item.title, 400)
                    } else {
                        clip_chars(&item.description, 500)
                    },
                    url: if item.link.is_empty() {
                        None
                    } else {
                        Some(item.link.clone())
                    },
                    iocs: cves.clone(),
                    cves,
                    tags,
                    ..Default::default()
                }
            })
            .collect())
    }
}

pub const PT_ANALYTICS: BlogRssConfig = BlogRssConfig {
    id: "pt_analytics",
    label: "Positive Technologies (RSS)",
    region: "RU",
    default_url: "https://www.ptsecurity.com/ru-ru/about/news/rss/",
    default_path: "/opt/aegis/feeds/pt-analytics-rss.xml",
    env_url: "PT_ANALYTICS_RSS_URL",
    env_path: "PT_ANALYTICS_RSS_PATH",
    extra_tags: &["pt", "analytics"],
};

pub const BI_ZONE: BlogRssConfig = BlogRssConfig {
    id: "bi_zone",
    label: "BI.ZONE Expertise (RSS)",
    region: "RU",
    default_url: "",
    default_path: "/opt/aegis/feeds/bi-zone-rss.xml",
    env_url: "BI_ZONE_RSS_URL",
    env_path: "BI_ZONE_RSS_PATH",
    extra_tags: &["bi-zone", "expertise"],
};

pub const FACCT: BlogRssConfig = BlogRssConfig {
    id: "facct",
    label: "FACCT (RSS)",
    region: "RU",
    default_url: "",
    default_path: "/opt/aegis/feeds/facct-rss.xml",
    env_url: "FACCT_RSS_URL",
    env_path: "FACCT_RSS_PATH",
    extra_tags: &["facct", "threat-research"],
};

pub const RT_SOLAR: BlogRssConfig = BlogRssConfig {
    id: "rt_solar",
    label: "RT-Solar (RSS)",
    region: "RU",
    default_url: "",
    default_path: "/opt/aegis/feeds/rt-solar-rss.xml",
    env_url: "RT_SOLAR_RSS_URL",
    env_path: "RT_SOLAR_RSS_PATH",
    extra_tags: &["rt-solar", "solar"],
};
