//! Scout v0.5 — разведка только по БДУ ФСТЭК (https://bdu.fstec.ru).
//! Парсит HTML-списки уязвимостей (критический / высокий уровень опасности).

use crate::knowledge_item::{KnowledgeItem, KnowledgeType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const BDU_BASE: &str = "https://bdu.fstec.ru";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BduVulnerability {
    pub id: String,
    pub bdu_id: String,
    pub title: String,
    pub severity: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
}

impl BduVulnerability {
    pub fn to_knowledge_item(&self) -> KnowledgeItem {
        let now = chrono::Utc::now().timestamp();
        let confidence = if self.severity == "critical" { 0.95 } else { 0.85 };
        let content = format!(
            "BDU ID: {}\nSeverity: {}\nURL: {}\n\n{}",
            self.bdu_id, self.severity, self.url, self.title
        );
        KnowledgeItem {
            id: uuid::Uuid::new_v4().to_string(),
            item_type: KnowledgeType::Black,
            content,
            summary: Some(format!("{} — {}", self.bdu_id, clip(&self.title, 200))),
            source: "fstec_bdu".to_string(),
            confidence,
            verified_by: vec!["scout_v0.5".into(), "fstec_bdu".into()],
            tags: vec![
                "scout".into(),
                "fstec".into(),
                "bdu".into(),
                self.severity.clone(),
            ],
            related_iocs: vec![self.bdu_id.clone()],
            first_seen: now,
            last_seen: now,
            embedding_id: None,
            content_hash: String::new(),
            feedback: None,
        }
    }
}

/// Загружает уязвимости уровня **critical** и **high** с bdu.fstec.ru.
pub async fn fetch_high_and_critical(limit: usize) -> Result<Vec<BduVulnerability>, String> {
    let danger_html = fetch_html(&format!("{}/vul/danger", BDU_BASE)).await?;
    let main_html = fetch_html(&format!("{}/vul", BDU_BASE)).await?;

    let mut by_id: HashMap<String, BduVulnerability> = HashMap::new();
    for v in parse_list_html(&danger_html) {
        by_id.entry(v.id.clone()).or_insert(v);
    }
    for v in parse_list_html(&main_html) {
        by_id.entry(v.id.clone()).or_insert(v);
    }

    let mut out: Vec<BduVulnerability> = by_id.into_values().collect();
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out.truncate(limit.max(1).min(100));
    Ok(out)
}

/// Загрузка HTML через системный `curl` (стабильно на VPS; reqwest/rustls к bdu.fstec.ru часто падает).
async fn fetch_html(url: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("curl")
        .args([
            "-skL",
            "--max-time",
            "25",
            "-A",
            USER_AGENT,
            "-H",
            "Accept: text/html,application/xhtml+xml",
            "-H",
            "Accept-Language: ru-RU,ru;q=0.9",
            "-H",
            &format!("Referer: {}/", BDU_BASE),
            url,
        ])
        .output()
        .await
        .map_err(|e| format!("curl spawn {}: {}", url, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "curl exit {} for {}: {}",
            output.status,
            url,
            stderr.trim()
        ));
    }

    let body = String::from_utf8_lossy(&output.stdout).into_owned();
    if body.len() < 500 {
        return Err(format!("empty or blocked response from {}", url));
    }
    Ok(body)
}

/// Парсит строки таблицы со списком уязвимостей.
pub fn parse_list_html(html: &str) -> Vec<BduVulnerability> {
    let mut out = Vec::new();
    for row in extract_rows(html) {
        if let Some(v) = parse_row(&row) {
            if v.severity == "critical" || v.severity == "high" {
                out.push(v);
            }
        }
    }
    out
}

fn extract_rows(html: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<tr") {
        rest = &rest[start..];
        let Some(end) = rest.find("</tr>") else { break };
        let row = &rest[..end + 5];
        rest = &rest[end + 5..];
        if row.contains("/vul/20") {
            rows.push(row.to_string());
        }
    }
    rows
}

fn parse_row(row: &str) -> Option<BduVulnerability> {
    let id = extract_vul_id(row)?;

    let severity = if row.contains("bsc-critical") {
        "critical".to_string()
    } else if row.contains("bsc-high") {
        "high".to_string()
    } else {
        return None;
    };

    let bdu_id = format!("BDU:{}", id);

    let title = extract_title(row).unwrap_or_else(|| format!("Уязвимость {}", bdu_id));

    let published = extract_published(row);

    Some(BduVulnerability {
        url: format!("{}/vul/{}", BDU_BASE, id),
        id,
        bdu_id,
        title,
        severity,
        published,
    })
}

fn extract_title(row: &str) -> Option<String> {
    if let Some(i) = row.find("data-content=\"") {
        let start = i + 14;
        let rest = &row[start..];
        let end = rest.find('"')?;
        return Some(html_unescape(&rest[..end]));
    }
    if let Some(i) = row.find("<h5") {
        let rest = &row[i..];
        if let Some(gt) = rest.find('>') {
            let inner = &rest[gt + 1..];
            if let Some(lt) = inner.find('<') {
                let t = html_unescape(inner[..lt].trim());
                if t.len() > 10 {
                    return Some(t);
                }
            }
        }
    }
    None
}

fn extract_vul_id(row: &str) -> Option<String> {
    let needle = "/vul/20";
    let start = row.find(needle)? + "/vul/".len();
    let digits: String = row[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    if digits.len() >= 10 && digits.starts_with("20") {
        Some(digits)
    } else {
        None
    }
}

fn extract_published(row: &str) -> Option<String> {
    for marker in ["<span>", "<span class=\"d-table hidden-xs\"><span>"] {
        if let Some(i) = row.find(marker) {
            let rest = &row[i + marker.len()..];
            let end = rest.find("</span>")?;
            let date = rest[..end].trim();
            if date.len() == 10 && date.as_bytes().get(2) == Some(&b'.') {
                return Some(date.to_string());
            }
        }
    }
    None
}

fn html_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_critical_row() {
        let row = r#"<tr>
        <td><h4><a href="/vul/2026-06427">BDU:2026-06427</a></h4></td>
        <td><div class="name"><h5 data-content="Уязвимость vm2 NPM">Уязвимость vm2 NPM, критическая</h5></motion></div></td>
        <td><motion class="bsc bsc-critical"></div><span class="d-table"><span>01.05.2026</span></span></td>
        </tr>"#;
        let v = parse_row(row).expect("row");
        assert_eq!(v.severity, "critical");
        assert_eq!(v.bdu_id, "BDU:2026-06427");
        assert_eq!(v.published.as_deref(), Some("01.05.2026"));
    }

    #[test]
    fn skips_medium_row() {
        let row = r#"<tr><td><a href="/vul/2026-00001">BDU:2026-00001</a></td>
        <td><h5>Test</h5></td><td><motion class="bsc bsc-middle"></div></td></tr>"#;
        assert!(parse_row(row).is_none());
    }
}
