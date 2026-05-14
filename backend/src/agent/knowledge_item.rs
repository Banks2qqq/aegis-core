use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeFeedback {
    Useful,
    NotUseful,
    FalsePositive,
    NeedsReview,
}

impl KnowledgeFeedback {
    pub fn as_str(&self) -> &'static str {
        match self {
            KnowledgeFeedback::Useful => "useful",
            KnowledgeFeedback::NotUseful => "not_useful",
            KnowledgeFeedback::FalsePositive => "false_positive",
            KnowledgeFeedback::NeedsReview => "needs_review",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "useful" => Some(KnowledgeFeedback::Useful),
            "not_useful" | "not-useful" | "notuseful" => Some(KnowledgeFeedback::NotUseful),
            "false_positive" | "false-positive" | "falsepositive" => Some(KnowledgeFeedback::FalsePositive),
            "needs_review" | "needs-review" | "needsreview" => Some(KnowledgeFeedback::NeedsReview),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KnowledgeType {
    White,
    Black,
    Hypothesis,
    TTP,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItem {
    pub id: String,                    // uuid
    pub item_type: KnowledgeType,      // White | Black | Hypothesis | TTP
    pub content: String,
    pub summary: Option<String>,
    pub source: String,                // "nvd", "cisa", "darknet-heuristic", "internal", etc.
    pub confidence: f64,               // 0.0 - 1.0
    pub verified_by: Vec<String>,      // ["critic", "inquisitor", "human"]
    pub tags: Vec<String>,
    pub related_iocs: Vec<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub embedding_id: Option<String>,  // id в Qdrant
    #[serde(default)]
    pub content_hash: String,          // хэш содержимого для дедупликации и дельта-синхронизации
    /// Оценка полезности после ingest (human loop).
    #[serde(default)]
    pub feedback: Option<KnowledgeFeedback>,
}

