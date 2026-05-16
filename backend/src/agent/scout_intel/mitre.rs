//! MITRE ATT&CK technique mapping (embedded map + keyword match).

use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, Deserialize)]
struct MitreEntry {
    id: String,
    name: String,
    tactic: String,
    keywords: Vec<String>,
}

static MITRE_MAP: LazyLock<Vec<MitreEntry>> = LazyLock::new(|| {
    let raw = include_str!("data/mitre_map.json");
    serde_json::from_str(raw).unwrap_or_default()
});

#[derive(Debug, Clone, Default)]
pub struct MitreMatch {
    pub techniques: Vec<String>,
    pub technique_names: Vec<String>,
    pub tactics: Vec<String>,
}

pub fn match_mitre_text(blob: &str) -> MitreMatch {
    let lower = blob.to_lowercase();
    let mut out = MitreMatch::default();
    for e in MITRE_MAP.iter() {
        if e.keywords.iter().any(|k| lower.contains(k)) {
            if !out.techniques.contains(&e.id) {
                out.techniques.push(e.id.clone());
                out.technique_names.push(e.name.clone());
            }
            if !out.tactics.contains(&e.tactic) {
                out.tactics.push(e.tactic.clone());
            }
        }
    }
    out
}

pub fn tactic_to_tag(tactic: &str) -> String {
    tactic.replace('_', "-")
}
