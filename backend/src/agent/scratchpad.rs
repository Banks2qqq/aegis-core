use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Гипотеза агента
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub statement: String,
    pub confidence: f64,      // 0.0 – 1.0
    pub evidence: Vec<String>, // что подтверждает
    pub created_at: i64,
}

/// Артефакт (скриншот, кусок кода, лог)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub artifact_type: String, // "screenshot", "log", "code", "network_capture"
    pub content: String,
    pub source: String,        // откуда получен
    pub timestamp: i64,
}

/// Наблюдение агента за один шаг
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub iteration: u32,
    pub thought: String,
    pub action: String,
    pub result: String,
    pub success: bool,
}

/// Scratchpad — оперативная память агента на время миссии
#[derive(Debug, Clone)]
pub struct Scratchpad {
    pub mission_id: String,
    pub observations: Vec<Observation>,
    pub hypotheses: Vec<Hypothesis>,
    pub artifacts: Vec<Artifact>,
    pub correlation_map: HashMap<String, Vec<String>>, // связь "артефакт → гипотезы"
    #[allow(dead_code)]
    created_at: i64,
}

impl Scratchpad {
    pub fn new(mission_id: &str) -> Self {
        Self {
            mission_id: mission_id.to_string(),
            observations: Vec::new(),
            hypotheses: Vec::new(),
            artifacts: Vec::new(),
            correlation_map: HashMap::new(),
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Добавить наблюдение
    pub fn observe(&mut self, obs: Observation) {
        self.observations.push(obs);
    }

    /// Добавить гипотезу
    pub fn hypothesize(&mut self, statement: &str, confidence: f64) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.hypotheses.push(Hypothesis {
            id: id.clone(),
            statement: statement.to_string(),
            confidence,
            evidence: Vec::new(),
            created_at: chrono::Utc::now().timestamp(),
        });
        id
    }

    /// Добавить доказательство к гипотезе
    pub fn add_evidence(&mut self, hypothesis_id: &str, evidence: &str) {
        if let Some(h) = self.hypotheses.iter_mut().find(|h| h.id == hypothesis_id) {
            h.evidence.push(evidence.to_string());
            h.confidence = (h.confidence + 0.1).min(1.0);
        }
    }

    /// Сохранить артефакт
    pub fn store_artifact(&mut self, artifact_type: &str, content: &str, source: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.artifacts.push(Artifact {
            id: id.clone(),
            artifact_type: artifact_type.to_string(),
            content: content.to_string(),
            source: source.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        });
        id
    }

    /// Связать артефакт с гипотезой
    pub fn correlate(&mut self, artifact_id: &str, hypothesis_id: &str) {
        self.correlation_map
            .entry(artifact_id.to_string())
            .or_default()
            .push(hypothesis_id.to_string());
    }

    /// Найти гипотезы, связанные с артефактом
    pub fn get_related_hypotheses(&self, artifact_id: &str) -> Vec<&Hypothesis> {
        let ids = self.correlation_map.get(artifact_id);
        match ids {
            Some(ids) => self
                .hypotheses
                .iter()
                .filter(|h| ids.contains(&h.id))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Получить самые уверенные гипотезы
    pub fn top_hypotheses(&self, limit: usize) -> Vec<&Hypothesis> {
        let mut sorted: Vec<&Hypothesis> = self.hypotheses.iter().collect();
        sorted.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        sorted.truncate(limit);
        sorted
    }

    /// Получить суммарный контекст для следующего шага ReAct
    pub fn get_context_for_llm(&self) -> String {
        let mut ctx = String::new();

        if !self.hypotheses.is_empty() {
            ctx.push_str("## ТЕКУЩИЕ ГИПОТЕЗЫ\n");
            for h in self.top_hypotheses(3) {
                ctx.push_str(&format!(
                    "- [{}] {} (уверенность: {:.0}%)\n",
                    h.id,
                    h.statement,
                    h.confidence * 100.0
                ));
            }
        }

        if !self.artifacts.is_empty() {
            ctx.push_str("\n## СОБРАННЫЕ АРТЕФАКТЫ\n");
            for a in &self.artifacts {
                ctx.push_str(&format!(
                    "- [{}] {} (источник: {})\n",
                    a.id, a.artifact_type, a.source
                ));
            }
        }

        if !self.observations.is_empty() {
            ctx.push_str("\n## ИСТОРИЯ НАБЛЮДЕНИЙ\n");
            for o in &self.observations {
                ctx.push_str(&format!(
                    "[Шаг {}] {} — {}\n",
                    o.iteration,
                    if o.success { "✅" } else { "❌" },
                    o.result
                ));
            }
        }

        if ctx.is_empty() {
            ctx = "Память пуста. Это первый шаг миссии.".into();
        }

        ctx
    }

    /// Проверить, есть ли корреляция с предыдущей миссией
    pub fn find_correlations_with(&self, other: &Scratchpad) -> Vec<String> {
        let mut correlations = Vec::new();
        for a in &self.artifacts {
            for oa in &other.artifacts {
                if a.source == oa.source || a.content.contains(&oa.content) {
                    correlations.push(format!(
                        "Корреляция: артефакт '{}' (миссия {}) ↔ артефакт '{}' (миссия {})",
                        a.id, self.mission_id, oa.id, other.mission_id
                    ));
                }
            }
        }
        correlations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scratchpad_hypothesis_lifecycle() {
        let mut sp = Scratchpad::new("test-mission-1");
        let h_id = sp.hypothesize("Сервер уязвим к CVE-2024-1234", 0.6);
        sp.add_evidence(&h_id, "Обнаружена версия nginx 1.25.0");
        let top = sp.top_hypotheses(1);
        assert_eq!(top.len(), 1);
        assert!(top[0].confidence > 0.6);
    }

    #[test]
    fn test_correlation_between_missions() {
        let mut sp1 = Scratchpad::new("mission-1");
        sp1.store_artifact("log", "error: connection refused on port 6379", "server-a");

        let mut sp2 = Scratchpad::new("mission-2");
        sp2.store_artifact("log", "error: connection refused on port 6379", "server-a");

        let corr = sp1.find_correlations_with(&sp2);
        assert!(!corr.is_empty());
    }
}