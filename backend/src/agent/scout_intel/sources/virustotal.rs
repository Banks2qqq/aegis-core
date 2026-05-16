//! VirusTotal file lookups (free API). Seeds hashes from OTX pulse indicators.

use async_trait::async_trait;
use serde_json::Value;
use tokio::time::{sleep, Duration};

use super::http_util::{clip_chars, get_json};
use super::{ScoutFinding, ScoutSource, SourceMeta};

/// Free tier: ~4 lookups / minute; keep under hub per-source timeout (35s).
const VT_MAX_LOOKUPS: usize = 2;
const VT_LOOKUP_INTERVAL_SECS: u64 = 12;

pub struct VirusTotalSource;

fn api_key() -> Result<String, String> {
    match std::env::var("VT_API_KEY") {
        Ok(k) if !k.trim().is_empty() => Ok(k.trim().to_string()),
        _ => Err("пропуск: задайте VT_API_KEY в /etc/aegis/agent.env".into()),
    }
}

fn otx_key() -> Result<String, String> {
    match std::env::var("OTX_API_KEY") {
        Ok(k) if !k.trim().is_empty() => Ok(k.trim().to_string()),
        _ => Err("VT: нужен OTX_API_KEY для seed-хэшей".into()),
    }
}

fn is_hash_indicator(indicator_type: &str) -> bool {
    let t = indicator_type.to_lowercase();
    t.contains("hash") || t.contains("sha") || t == "md5"
}

async fn seed_hashes_from_otx(limit: usize) -> Result<Vec<String>, String> {
    let otx = otx_key()?;
    let hdr = [("X-OTX-API-KEY", otx.as_str())];
    let pulses = get_json(
        &format!(
            "https://otx.alienvault.com/api/v1/pulses/subscribed?limit={}",
            2.min(limit.max(1))
        ),
        &hdr,
    )
    .await?;
    let pulse_ids: Vec<String> = pulses
        .get("results")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| p.get("id").and_then(|x| x.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let mut hashes = Vec::new();
    for pid in pulse_ids {
        let ind = get_json(
            &format!(
                "https://otx.alienvault.com/api/v1/pulses/{pid}/indicators?limit=20"
            ),
            &hdr,
        )
        .await?;
        if let Some(rows) = ind.get("results").and_then(|r| r.as_array()) {
            for row in rows {
                let itype = row.get("type").and_then(|x| x.as_str()).unwrap_or("");
                if !is_hash_indicator(itype) {
                    continue;
                }
                if let Some(h) = row.get("indicator").and_then(|x| x.as_str()) {
                    let h = h.trim();
                    if h.len() >= 32 {
                        hashes.push(h.to_string());
                    }
                }
            }
        }
        if hashes.len() >= limit {
            break;
        }
    }
    hashes.sort();
    hashes.dedup();
    hashes.truncate(limit);
    if hashes.is_empty() {
        return Err("VT: в OTX pulses нет file-hash индикаторов для lookup".into());
    }
    Ok(hashes)
}

fn severity_from_stats(malicious: i64, total: i64) -> &'static str {
    if total <= 0 {
        return "medium";
    }
    let ratio = malicious as f64 / total as f64;
    if malicious >= 30 || ratio >= 0.5 {
        "critical"
    } else if malicious >= 10 || ratio >= 0.25 {
        "high"
    } else if malicious >= 3 {
        "medium"
    } else {
        "low"
    }
}

fn file_report_to_finding(data: &Value, hash: &str) -> Option<ScoutFinding> {
    let attrs = data.get("attributes")?;
    let sha256 = attrs
        .get("sha256")
        .and_then(|x| x.as_str())
        .unwrap_or(hash);
    let name = attrs
        .get("meaningful_name")
        .and_then(|x| x.as_str())
        .or_else(|| {
            attrs
                .get("names")
                .and_then(|n| n.as_array())
                .and_then(|arr| arr.first())
                .and_then(|x| x.as_str())
        })
        .unwrap_or(sha256);
    let stats = attrs.get("last_analysis_stats")?;
    let malicious = stats.get("malicious").and_then(|x| x.as_i64()).unwrap_or(0);
    let total = ["malicious", "suspicious", "undetected", "harmless"]
        .iter()
        .filter_map(|k| stats.get(*k).and_then(|x| x.as_i64()))
        .sum::<i64>();
    let tags: Vec<String> = attrs
        .get("tags")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let sev = severity_from_stats(malicious, total);
    let mut tag_list = vec!["virustotal".into(), "file".into(), "vt_lookup".into(), "otx_seed".into()];
    tag_list.extend(tags);
    Some(ScoutFinding {
        id: uuid::Uuid::new_v4().to_string(),
        source_id: "virustotal".into(),
        source_label: "VirusTotal".into(),
        title: format!("{name} — {malicious}/{total} VT detections"),
        severity: sev.into(),
        summary: format!(
            "VT lookup {} | malicious={} | {}",
            sha256,
            malicious,
            clip_chars(&tag_list.join(", "), 100)
        ),
        url: Some(format!("https://www.virustotal.com/gui/file/{sha256}")),
        iocs: vec![sha256.to_string()],
        cves: Vec::new(),
        mitre_techniques: Vec::new(),
        tags: tag_list,
        structured: super::super::structured::ScoutIocsStructured {
            hashes: vec![sha256.to_string()],
            ..Default::default()
        },
        ..Default::default()
    })
}

#[async_trait]
impl ScoutSource for VirusTotalSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "virustotal",
            label: "VirusTotal",
            region: "INTL",
            needs_api_key: true,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let key = api_key()?;
        let lookup_n = limit.min(VT_MAX_LOOKUPS);
        let hashes = seed_hashes_from_otx(lookup_n * 2).await?;
        let mut out = Vec::new();
        let mut looked = 0usize;

        for hash in hashes {
            if looked >= lookup_n {
                break;
            }
            if looked > 0 {
                sleep(Duration::from_secs(VT_LOOKUP_INTERVAL_SECS)).await;
            }
            let url = format!("https://www.virustotal.com/api/v3/files/{hash}");
            let v = match get_json(&url, &[("x-apikey", key.as_str())]).await {
                Ok(j) => j,
                Err(e) => {
                    tracing::warn!("VT lookup {} failed: {}", hash, e);
                    continue;
                }
            };
            looked += 1;
            if let Some(f) = v
                .get("data")
                .and_then(|d| file_report_to_finding(d, &hash))
            {
                out.push(f);
            }
        }

        if out.is_empty() {
            return Err(
                "VirusTotal: lookup не дал результатов (rate limit или хэши не в VT)".into(),
            );
        }
        Ok(out)
    }
}
