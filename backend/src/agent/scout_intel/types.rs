//! Normalized scout finding (all sources → one shape for KB + UX).

use sha2::{Digest, Sha256};

use crate::knowledge_item::{KnowledgeItem, KnowledgeType};

use super::structured::ScoutIocsStructured;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ScoutFinding {
    pub id: String,
    pub source_id: String,
    pub source_label: String,
    pub title: String,
    pub severity: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Flat IOC list (legacy + search); mirrors structured buckets.
    pub iocs: Vec<String>,
    pub cves: Vec<String>,
    pub mitre_techniques: Vec<String>,
    pub mitre_tactics: Vec<String>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub structured: ScoutIocsStructured,
    #[serde(default)]
    pub content_hash: String,
}

impl ScoutFinding {
    pub fn compute_content_hash(&self) -> String {
        let primary = self
            .structured
            .primary_key()
            .unwrap_or_else(|| self.title.clone());
        let mut hasher = Sha256::new();
        hasher.update(self.source_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(primary.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.title.trim().to_lowercase().as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn to_knowledge_item(&self) -> KnowledgeItem {
        let now = chrono::Utc::now().timestamp();
        let confidence = match self.severity.as_str() {
            "critical" => 0.95,
            "high" => 0.85,
            "medium" => 0.7,
            _ => 0.55,
        };
        let mut tags = vec![
            "scout".into(),
            "scout_2".into(),
            self.source_id.clone(),
            self.severity.clone(),
        ];
        tags.extend(self.tags.clone());
        tags.extend(
            self.mitre_tactics
                .iter()
                .map(|t| format!("tactic:{t}"))
                .collect::<Vec<_>>(),
        );
        tags.extend(
            self.mitre_techniques
                .iter()
                .map(|t| format!("mitre:{t}"))
                .collect::<Vec<_>>(),
        );

        let content = format!(
            "Source: {} ({})\nSeverity: {}\nTags: {}\nTactics: {}\nTechniques: {}\n\n{}\n\n--- IOC ---\nIP: {}\nDomains: {}\nURLs: {}\nHashes: {}\nCVEs: {}\n\nRef: {}",
            self.source_label,
            self.source_id,
            self.severity,
            if self.tags.is_empty() { "—".into() } else { self.tags.join(", ") },
            if self.mitre_tactics.is_empty() { "—".into() } else { self.mitre_tactics.join(", ") },
            if self.mitre_techniques.is_empty() { "—".into() } else { self.mitre_techniques.join(", ") },
            self.summary,
            if self.structured.ips.is_empty() { "—".into() } else { self.structured.ips.join(", ") },
            if self.structured.domains.is_empty() { "—".into() } else { self.structured.domains.join(", ") },
            if self.structured.urls.is_empty() { "—".into() } else { self.structured.urls.join(", ") },
            if self.structured.hashes.is_empty() { "—".into() } else { self.structured.hashes.join(", ") },
            if self.structured.cves.is_empty() { "—".into() } else { self.structured.cves.join(", ") },
            self.url.as_deref().unwrap_or("—"),
        );

        let hash = if self.content_hash.is_empty() {
            self.compute_content_hash()
        } else {
            self.content_hash.clone()
        };

        KnowledgeItem {
            id: self.id.clone(),
            item_type: KnowledgeType::Black,
            content,
            summary: Some(format!(
                "[{}] {} | tags: {}",
                self.severity.to_uppercase(),
                clip(&self.title, 120),
                clip(&self.tags.join(","), 60)
            )),
            source: self.source_id.clone(),
            confidence,
            verified_by: vec!["scout_intel".into(), "scout_2_enriched".into()],
            tags,
            related_iocs: self.structured.all_flat(),
            first_seen: now,
            last_seen: now,
            embedding_id: None,
            content_hash: hash,
            feedback: None,
        }
    }
}

fn clip(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}
