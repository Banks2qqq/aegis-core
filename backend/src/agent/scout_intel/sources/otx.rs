//! AlienVault OTX — subscribed pulses (requires OTX_API_KEY).

use async_trait::async_trait;
use serde_json::Value;

use super::http_util::{clip_chars, get_json};
use super::{ScoutFinding, ScoutSource, SourceMeta};

pub struct OtxSource;

fn api_key() -> Result<String, String> {
    match std::env::var("OTX_API_KEY") {
        Ok(k) if !k.trim().is_empty() => Ok(k.trim().to_string()),
        _ => Err("пропуск: задайте OTX_API_KEY в /etc/aegis/agent.env".into()),
    }
}

fn severity_from_tags(tags: &[String]) -> &'static str {
    let joined = tags.join(" ").to_lowercase();
    if joined.contains("ransomware") || joined.contains("apt") || joined.contains("0day") {
        "critical"
    } else if joined.contains("malware") || joined.contains("trojan") || joined.contains("exploit") {
        "high"
    } else {
        "medium"
    }
}

fn pulse_to_finding(pulse: &Value) -> Option<ScoutFinding> {
    let id = pulse.get("id")?.as_str()?;
    let name = pulse.get("name").and_then(|x| x.as_str()).unwrap_or("OTX pulse");
    let desc = pulse
        .get("description")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let tags: Vec<String> = pulse
        .get("tags")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let adversary = pulse
        .get("adversary")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty());
    let mut summary = clip_chars(desc, 400);
    if let Some(adv) = adversary {
        summary = format!("Adversary: {adv} | {summary}");
    }
    let mut tag_list = vec!["otx".into(), "alienvault".into()];
    tag_list.extend(tags.clone());
    let sev = severity_from_tags(&tag_list);
    Some(ScoutFinding {
        id: uuid::Uuid::new_v4().to_string(),
        source_id: "otx".into(),
        source_label: "AlienVault OTX".into(),
        title: clip_chars(name, 120),
        severity: sev.into(),
        summary,
        url: Some(format!("https://otx.alienvault.com/pulse/{id}")),
        iocs: vec![format!("otx:pulse:{id}")],
        cves: Vec::new(),
        mitre_techniques: Vec::new(),
        tags: tag_list,
        ..Default::default()
    })
}

#[async_trait]
impl ScoutSource for OtxSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "otx",
            label: "AlienVault OTX",
            region: "INTL",
            needs_api_key: true,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let key = api_key()?;
        let url = format!(
            "https://otx.alienvault.com/api/v1/pulses/subscribed?limit={}&page=1",
            limit.min(25)
        );
        let v = get_json(&url, &[("X-OTX-API-KEY", key.as_str())]).await?;
        let results = v
            .get("results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| "OTX: пустой ответ (нет results)".to_string())?;
        let mut out = Vec::new();
        for pulse in results.iter().take(limit) {
            if let Some(f) = pulse_to_finding(pulse) {
                out.push(f);
            }
        }
        if out.is_empty() {
            return Err("OTX: подписки пусты — добавьте pulses в OTX или проверьте ключ".into());
        }
        Ok(out)
    }
}
