//! abuse.ch — ThreatFox, URLhaus, MalwareBazaar (Auth-Key required since 2024).

use async_trait::async_trait;
use serde_json::Value;

use super::http_util::{get_json, post_form, post_json};
use super::{ScoutFinding, ScoutSource, SourceMeta};

fn abuse_headers() -> Vec<(String, String)> {
    match std::env::var("ABUSECH_API_KEY") {
        Ok(k) if !k.trim().is_empty() => vec![("Auth-Key".into(), k.trim().to_string())],
        _ => Vec::new(),
    }
}

fn header_refs(h: &[(String, String)]) -> Vec<(&str, &str)> {
    h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

fn check_abuse_payload(v: &Value) -> Result<(), String> {
    if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
        if err.to_lowercase().contains("unauthorized") {
            return Err(abuse_auth_message());
        }
    }
    Ok(())
}

fn abuse_auth_message() -> String {
    if std::env::var("ABUSECH_API_KEY")
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
    {
        "abuse.ch: Auth-Key отклонён — проверьте ABUSECH_API_KEY".into()
    } else {
        "пропуск: задайте ABUSECH_API_KEY (https://auth.abuse.ch/)".into()
    }
}

async fn post_abuse(url: &str, body: Value) -> Result<Value, String> {
    let hdrs = abuse_headers();
    let refs = header_refs(&hdrs);
    let v = match post_json(url, body, &refs).await {
        Err(e) if e.contains("Unauthorized") => return Err(abuse_auth_message()),
        r => r?,
    };
    check_abuse_payload(&v)?;
    Ok(v)
}

async fn get_abuse(url: &str) -> Result<Value, String> {
    let hdrs = abuse_headers();
    let refs = header_refs(&hdrs);
    let v = match get_json(url, &refs).await {
        Err(e) if e.contains("Unauthorized") => return Err(abuse_auth_message()),
        r => r?,
    };
    check_abuse_payload(&v)?;
    Ok(v)
}

fn severity_from_confidence(c: i64) -> &'static str {
    if c >= 80 {
        "high"
    } else if c >= 50 {
        "medium"
    } else {
        "low"
    }
}

pub struct ThreatFoxSource;

#[async_trait]
impl ScoutSource for ThreatFoxSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "threatfox",
            label: "ThreatFox (abuse.ch)",
            region: "INTL",
            needs_api_key: false,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let v = post_abuse(
            "https://threatfox-api.abuse.ch/api/v1/",
            serde_json::json!({ "query": "get_iocs", "days": 3 }),
        )
        .await?;
        let data = v
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| "ThreatFox: no data".to_string())?;
        let mut out = Vec::new();
        for row in data.iter().take(limit) {
            let ioc = row.get("ioc").and_then(|x| x.as_str()).unwrap_or("?");
            let malware = row
                .get("malware_printable")
                .or_else(|| row.get("malware"))
                .and_then(|x| x.as_str())
                .unwrap_or("unknown");
            let conf = row
                .get("confidence_level")
                .and_then(|x| x.as_i64())
                .unwrap_or(50);
            let sev = severity_from_confidence(conf);
            let ioc_type = row
                .get("ioc_type")
                .and_then(|x| x.as_str())
                .unwrap_or("ioc");
            out.push(ScoutFinding {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: "threatfox".into(),
                source_label: "ThreatFox".into(),
                title: format!("{ioc_type} {ioc} — {malware}"),
                severity: sev.into(),
                summary: format!(
                    "ThreatFox IOC={ioc} type={ioc_type} malware={malware} conf={conf}"
                ),
                url: Some(format!("https://threatfox.abuse.ch/ioc/{ioc}/")),
                iocs: vec![ioc.to_string()],
                cves: Vec::new(),
                mitre_techniques: Vec::new(),
                tags: vec!["threatfox".into(), "abuse.ch".into(), "c2".into()],
                ..Default::default()
            });
        }
        Ok(out)
    }
}

pub struct UrlhausSource;

#[async_trait]
impl ScoutSource for UrlhausSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "urlhaus",
            label: "URLhaus (abuse.ch)",
            region: "INTL",
            needs_api_key: false,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let v = get_abuse(&format!(
            "https://urlhaus-api.abuse.ch/v1/urls/recent/limit/{}/",
            limit.min(50)
        ))
        .await?;
        let urls = v
            .get("urls")
            .and_then(|d| d.as_array())
            .ok_or_else(|| "URLhaus: no urls".to_string())?;
        let mut out = Vec::new();
        for row in urls.iter().take(limit) {
            let url = row.get("url").and_then(|x| x.as_str()).unwrap_or("?");
            let threat = row.get("threat").and_then(|x| x.as_str()).unwrap_or("unknown");
            let tags: Vec<String> = row
                .get("tags")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            out.push(ScoutFinding {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: "urlhaus".into(),
                source_label: "URLhaus".into(),
                title: format!("Malicious URL — {threat}"),
                severity: "high".into(),
                summary: format!("URLhaus url={url} threat={threat}"),
                url: Some(url.to_string()),
                iocs: vec![url.to_string()],
                cves: Vec::new(),
                mitre_techniques: Vec::new(),
                tags: {
                    let mut t = vec!["urlhaus".into(), "abuse.ch".into()];
                    t.extend(tags);
                    t
                },
                ..Default::default()
            });
        }
        Ok(out)
    }
}

pub struct MalwareBazaarSource;

#[async_trait]
impl ScoutSource for MalwareBazaarSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "malwarebazaar",
            label: "MalwareBazaar (abuse.ch)",
            region: "INTL",
            needs_api_key: false,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let hdrs = abuse_headers();
        let refs = header_refs(&hdrs);
        let v = match post_form(
            "https://mb-api.abuse.ch/api/v1/",
            &[("query", "get_recent"), ("selector", "time")],
            &refs,
        )
        .await
        {
            Err(e) if e.contains("Unauthorized") => return Err(abuse_auth_message()),
            r => r?,
        };
        check_abuse_payload(&v)?;
        if v.get("query_status").and_then(|x| x.as_str()) != Some("ok") {
            return Err(format!(
                "MalwareBazaar: {}",
                v.get("query_status")
                    .and_then(|x| x.as_str())
                    .unwrap_or("error")
            ));
        }
        let data = v
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| "MalwareBazaar: no data".to_string())?;
        let mut out = Vec::new();
        for row in data.iter().take(limit) {
            let hash = row
                .get("sha256_hash")
                .and_then(|x| x.as_str())
                .unwrap_or("?");
            let sig = row
                .get("signature")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown");
            let tags: Vec<String> = row
                .get("tags")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let sev = if tags.iter().any(|t| t.to_lowercase().contains("ransom")) {
                "critical"
            } else {
                "high"
            };
            out.push(ScoutFinding {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: "malwarebazaar".into(),
                source_label: "MalwareBazaar".into(),
                title: format!("Sample {sig}"),
                severity: sev.into(),
                summary: format!("MalwareBazaar sha256={hash} signature={sig}"),
                url: Some(format!("https://bazaar.abuse.ch/sample/{hash}/")),
                iocs: vec![hash.to_string()],
                cves: Vec::new(),
                mitre_techniques: Vec::new(),
                tags: {
                    let mut t = vec!["malwarebazaar".into(), "abuse.ch".into()];
                    t.extend(tags);
                    t
                },
                structured: super::super::structured::ScoutIocsStructured {
                    hashes: vec![hash.to_string()],
                    ..Default::default()
                },
                ..Default::default()
            });
        }
        Ok(out)
    }
}
