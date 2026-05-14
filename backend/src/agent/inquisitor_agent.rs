//! Inquisitor 2.0 — углублённая проверка структурированных знаний (KnowledgeItem).
//! LLM только через `crate::call_llm` (critical). При сбоях — эвристический fallback.

use crate::audit::AuditTrail;
use crate::config::AEGISConfig;
use crate::critic_agent::CriticEvaluation;
use crate::critic_agent::Verdict as CriticVerdict;
use crate::knowledge_item::{KnowledgeItem, KnowledgeType};
use crate::utils;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

const INQUISITOR_KNOWLEDGE_LLM_SECS: u64 = 28;

/// Вердикт Inquisitor 2.0 (knowledge path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InquisitorVerdict {
    Allow,
    Escalate,
    Block,
}

impl Serialize for InquisitorVerdict {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for InquisitorVerdict {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        InquisitorVerdict::parse(&s).ok_or_else(|| serde::de::Error::custom("unknown inquisitor verdict"))
    }
}

impl InquisitorVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            InquisitorVerdict::Allow => "ALLOW",
            InquisitorVerdict::Escalate => "ESCALATE",
            InquisitorVerdict::Block => "BLOCK",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "ALLOW" => Some(InquisitorVerdict::Allow),
            "ESCALATE" => Some(InquisitorVerdict::Escalate),
            "BLOCK" => Some(InquisitorVerdict::Block),
            _ => None,
        }
    }
}

/// Расширенная оценка Inquisitor для одного `KnowledgeItem`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InquisitorEvaluation {
    pub verdict: InquisitorVerdict,
    pub confidence: f64,
    pub risk_areas: Vec<String>,
    pub reasoning: String,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub suggested_actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct InquisitorEvalJson {
    verdict: String,
    confidence: f64,
    #[serde(default)]
    risk_areas: Vec<String>,
    reasoning: String,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    suggested_actions: Vec<String>,
}

/// Зависимости для `evaluate_knowledge` через `crate::call_llm`.
#[derive(Clone)]
pub struct InquisitorLlm {
    pub http: Client,
    pub api_key: String, // legacy fallback
    pub key_provider: Arc<dyn crate::key_provider::KeyProvider>,
    pub config: Arc<AEGISConfig>,
    pub local: Option<crate::local_llm::LocalLlmClient>,
}

/// Inquisitor 2.0 — per-item knowledge audit.
#[derive(Clone)]
pub struct Inquisitor {
    air_gapped: bool,
    llm: Option<Arc<InquisitorLlm>>,
    audit: Option<Arc<AuditTrail>>,
}

impl Inquisitor {
    pub fn new(air_gapped: bool) -> Self {
        Self {
            air_gapped,
            llm: None,
            audit: None,
        }
    }

    pub fn with_llm(mut self, llm: InquisitorLlm) -> Self {
        self.llm = Some(Arc::new(llm));
        self
    }

    pub fn with_audit(mut self, audit: Arc<AuditTrail>) -> Self {
        self.audit = Some(audit);
        self
    }

    /// Углублённая проверка одной находки; при сбое LLM — эвристика (graceful fallback).
    pub async fn evaluate_knowledge(
        &self,
        item: &KnowledgeItem,
        context: Option<&str>,
        critic_evaluation: Option<&CriticEvaluation>,
    ) -> Result<InquisitorEvaluation, String> {
        if self.air_gapped {
            let ev = heuristic_inquisitor_evaluation(item, critic_evaluation);
            self.log_fallback("air_gapped");
            self.log_evaluated(&ev, item, "heuristic");
            self.maybe_log_risk_detected(&ev, item);
            metrics_inquisitor_knowledge(&ev);
            return Ok(ev);
        }

        let Some(llm) = &self.llm else {
            let ev = heuristic_inquisitor_evaluation(item, critic_evaluation);
            self.log_fallback("no_llm_context");
            self.log_evaluated(&ev, item, "heuristic");
            self.maybe_log_risk_detected(&ev, item);
            metrics_inquisitor_knowledge(&ev);
            return Ok(ev);
        };

        let item_type = match item.item_type {
            KnowledgeType::White => "White",
            KnowledgeType::Black => "Black",
            KnowledgeType::Hypothesis => "Hypothesis",
            KnowledgeType::TTP => "TTP",
        };
        let verified = item.verified_by.join(", ");
        let ctx = context.unwrap_or("(none)");
        let body = clip(&item.content, 6000);
        let critic_note = critic_evaluation.map(describe_critic).unwrap_or_default();

        let system = "You are the Inquisitor: a senior security reviewer for AEGIS Threat Intelligence. \
You inspect structured knowledge (not executable commands) for contradictions, data poisoning, false positives, \
and unsafe ingestion impact. Output ONLY valid JSON, no markdown fences.";

        let user = format!(
            r#"Deeply audit this knowledge item.

Knowledge type: {item_type}
Verification trail (verified_by): {verified}
Source: {source}
Source confidence: {conf:.3}

Research / peer context:
{ctx}

Critic 2.0 summary (if any):
{critic_note}

Item text:
{body}

Return JSON exactly:
{{"verdict":"ALLOW"|"ESCALATE"|"BLOCK","confidence":0.0,"risk_areas":[],"reasoning":"4-8 sentences","flags":[],"suggested_actions":[]}}

risk_areas examples: "data_poisoning", "contradiction", "high_impact", "low_quality", "unverified_hypothesis", "source_bias".
flags examples: "needs_more_verification", "potential_false_positive", "stale_intel".
suggested_actions examples: "manual_review", "cross_check_with_nvd", "correlate_with_internal_telemetry".
"#,
            item_type = item_type,
            verified = verified,
            source = item.source,
            conf = item.confidence,
            ctx = ctx,
            critic_note = critic_note,
            body = body,
        );

        let fut = crate::call_llm(
            &llm.http,
            llm.key_provider.as_ref(),
            system,
            &user,
            llm.config.as_ref(),
            llm.local.as_ref(),
            true,
        );
        let raw = match timeout(Duration::from_secs(INQUISITOR_KNOWLEDGE_LLM_SECS), fut).await {
            Ok(Some(t)) if t != "[BLOCKED]" && !t.trim().is_empty() => t,
            _ => {
                let ev = heuristic_inquisitor_evaluation(item, critic_evaluation);
                self.log_fallback("llm_timeout_or_empty");
                self.log_evaluated(&ev, item, "heuristic");
                self.maybe_log_risk_detected(&ev, item);
                metrics_inquisitor_knowledge(&ev);
                return Ok(ev);
            }
        };

        let Some(slice) = extract_json_object_owned(&raw) else {
            let ev = heuristic_inquisitor_evaluation(item, critic_evaluation);
            self.log_fallback("bad_json_shape");
            self.log_evaluated(&ev, item, "heuristic");
            self.maybe_log_risk_detected(&ev, item);
            metrics_inquisitor_knowledge(&ev);
            return Ok(ev);
        };

        let parsed: InquisitorEvalJson = match serde_json::from_str(&slice) {
            Ok(v) => v,
            Err(_) => {
                let ev = heuristic_inquisitor_evaluation(item, critic_evaluation);
                self.log_fallback("json_parse");
                self.log_evaluated(&ev, item, "heuristic");
                self.maybe_log_risk_detected(&ev, item);
                metrics_inquisitor_knowledge(&ev);
                return Ok(ev);
            }
        };

        let verdict = InquisitorVerdict::parse(&parsed.verdict).unwrap_or(InquisitorVerdict::Escalate);
        let confidence = parsed.confidence.clamp(0.0, 1.0);
        let mut risk_areas: Vec<String> = parsed
            .risk_areas
            .into_iter()
            .map(|s| s.trim().to_lowercase().replace(' ', "_"))
            .filter(|s| !s.is_empty())
            .collect();
        let mut flags = parsed.flags;
        let suggested_actions = parsed.suggested_actions;
        let mut reasoning = parsed.reasoning.trim().to_string();
        if reasoning.is_empty() {
            reasoning = "Inquisitor 2.0: empty reasoning from model.".into();
        }

        if confidence < 0.45 && !flags.iter().any(|f| f == "high_uncertainty") {
            flags.push("high_uncertainty".into());
        }
        if matches!(verdict, InquisitorVerdict::Escalate | InquisitorVerdict::Block)
            && risk_areas.is_empty()
        {
            risk_areas.push("unspecified_risk".into());
        }

        let ev = InquisitorEvaluation {
            verdict,
            confidence,
            risk_areas,
            reasoning,
            flags,
            suggested_actions,
        };

        self.log_evaluated(&ev, item, "llm");
        self.maybe_log_risk_detected(&ev, item);
        metrics_inquisitor_knowledge(&ev);
        Ok(ev)
    }

    fn log_fallback(&self, reason: &str) {
        if let Some(a) = &self.audit {
            let _ = a.log_event(
                "inquisitor",
                &format!("inquisitor_fallback_used reason={}", reason),
                0.35,
                false,
            );
        }
    }

    fn log_evaluated(&self, ev: &InquisitorEvaluation, item: &KnowledgeItem, source: &str) {
        if let Some(a) = &self.audit {
            let _ = a.log_event(
                "inquisitor",
                &format!(
                    "inquisitor_evaluated_knowledge source={} id={} type={:?} verdict={} conf={:.2} risk_areas={:?} flags={:?}",
                    source,
                    clip(&item.id, 12),
                    item.item_type,
                    ev.verdict.as_str(),
                    ev.confidence,
                    ev.risk_areas,
                    ev.flags
                ),
                match ev.verdict {
                    InquisitorVerdict::Block => 0.92,
                    InquisitorVerdict::Escalate => 0.55,
                    InquisitorVerdict::Allow => 0.2,
                },
                true,
            );
        }
    }

    fn maybe_log_risk_detected(&self, ev: &InquisitorEvaluation, item: &KnowledgeItem) {
        let notable = !ev.risk_areas.is_empty()
            || matches!(
                ev.verdict,
                InquisitorVerdict::Block | InquisitorVerdict::Escalate
            );
        if !notable {
            return;
        }
        if let Some(a) = &self.audit {
            let _ = a.log_event(
                "inquisitor",
                &format!(
                    "inquisitor_risk_detected id={} type={:?} verdict={} risk_areas={:?} conf={:.2}",
                    clip(&item.id, 12),
                    item.item_type,
                    ev.verdict.as_str(),
                    ev.risk_areas,
                    ev.confidence
                ),
                match ev.verdict {
                    InquisitorVerdict::Block => 0.9,
                    InquisitorVerdict::Escalate => 0.6,
                    InquisitorVerdict::Allow => 0.35,
                },
                false,
            );
        }
    }
}

fn metrics_inquisitor_knowledge(ev: &InquisitorEvaluation) {
    crate::metrics::inquisitor_knowledge_verdict(ev.verdict.as_str());
}

fn describe_critic(c: &CriticEvaluation) -> String {
    format!(
        "verdict={} security_risk={:.2} utility={:.2} critic_conf={:.2} flags={:?}",
        c.verdict.as_str(),
        c.security_risk,
        c.utility,
        c.confidence,
        c.flags
    )
}

/// Рисковые зоны, усиливающие gate (слияние с bulk Inquisitor → ESCALATE минимум).
pub fn high_priority_risk_areas() -> &'static [&'static str] {
    &[
        "data_poisoning",
        "contradiction",
        "high_impact",
        "injection_risk",
        "unverified_hypothesis",
    ]
}

pub fn evaluation_requires_escalation(ev: &InquisitorEvaluation) -> bool {
    if evaluation_is_hard_block(ev) {
        return false;
    }
    matches!(ev.verdict, InquisitorVerdict::Escalate)
        || ev.risk_areas.iter().any(|r| {
            high_priority_risk_areas()
                .iter()
                .any(|h| r.eq_ignore_ascii_case(h))
        })
}

pub fn evaluation_is_hard_block(ev: &InquisitorEvaluation) -> bool {
    matches!(ev.verdict, InquisitorVerdict::Block)
        || ev
            .risk_areas
            .iter()
            .any(|r| r.eq_ignore_ascii_case("data_poisoning"))
}

fn heuristic_inquisitor_evaluation(
    item: &KnowledgeItem,
    critic: Option<&CriticEvaluation>,
) -> InquisitorEvaluation {
    let mut risk_areas = Vec::new();
    let mut flags = vec!["heuristic_fallback".to_string()];
    let n_vf = item.verified_by.len();
    let has_human = item.verified_by.iter().any(|v| v == "human");

    if let Some(c) = critic {
        if c.security_risk > 0.82 {
            risk_areas.push("high_impact".into());
        }
        if matches!(c.verdict, CriticVerdict::Escalate | CriticVerdict::Block) {
            risk_areas.push("contradiction".into());
        }
    }

    if matches!(item.item_type, KnowledgeType::Hypothesis) && !has_human {
        risk_areas.push("unverified_hypothesis".into());
        flags.push("needs_more_verification".into());
    }

    if item.confidence < 0.35 {
        risk_areas.push("low_quality".into());
    }

    if matches!(item.item_type, KnowledgeType::Black) && n_vf < 2 && !has_human {
        risk_areas.push("high_impact".into());
    }

    let (verdict, confidence, reasoning) = if let Some(c) = critic {
        if matches!(c.verdict, CriticVerdict::Block) {
            (
                InquisitorVerdict::Block,
                0.55,
                format!(
                    "Heuristic Inquisitor: Critic indicated BLOCK; type={:?}, source={}, verified_by={}.",
                    item.item_type,
                    clip(&item.source, 80),
                    n_vf
                ),
            )
        } else if !risk_areas.is_empty() || matches!(c.verdict, CriticVerdict::Escalate) {
            (
                InquisitorVerdict::Escalate,
                0.5,
                format!(
                    "Heuristic Inquisitor: elevated risk areas {:?}; Critic={}; item type={:?}.",
                    risk_areas,
                    c.verdict.as_str(),
                    item.item_type
                ),
            )
        } else if matches!(item.item_type, KnowledgeType::White) {
            (
                InquisitorVerdict::Allow,
                0.55,
                "Heuristic Inquisitor: White baseline with no critic red flags.".into(),
            )
        } else {
            (
                InquisitorVerdict::Escalate,
                0.45,
                format!(
                    "Heuristic Inquisitor: conservative pass for {:?} pending human correlation.",
                    item.item_type
                ),
            )
        }
    } else if matches!(item.item_type, KnowledgeType::White) && has_human {
        (
            InquisitorVerdict::Allow,
            0.5,
            "Heuristic Inquisitor: White + human verification.".into(),
        )
    } else if risk_areas.is_empty() {
        (
            InquisitorVerdict::Escalate,
            0.42,
            format!(
                "Heuristic Inquisitor: no Critic snapshot; type={:?}, verify_count={}.",
                item.item_type, n_vf
            ),
        )
    } else {
        (
            InquisitorVerdict::Escalate,
            0.48,
            format!(
                "Heuristic Inquisitor: risk_areas={:?}, type={:?}.",
                risk_areas, item.item_type
            ),
        )
    };

    if verdict != InquisitorVerdict::Allow {
        flags.push("needs_human_review".into());
    }

    InquisitorEvaluation {
        verdict,
        confidence,
        risk_areas,
        reasoning,
        flags,
        suggested_actions: vec!["manual_review".into(), "cross_check_sources".into()],
    }
}

fn clip(s: &str, max: usize) -> String {
    utils::clip(s, max)
}

#[allow(dead_code)]
fn extract_json_object(text: &str) -> Option<&str> {
    utils::extract_json_object(text)
}

fn extract_json_object_owned(text: &str) -> Option<String> {
    utils::extract_json_object_owned(text)
}
