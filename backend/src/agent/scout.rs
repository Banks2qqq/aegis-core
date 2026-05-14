use crate::audit::AuditTrail;
use crate::config::AEGISConfig;
use crate::knowledge_item::{KnowledgeItem, KnowledgeType};
use crate::local_llm::LocalLlmClient;
use crate::tool_registry::ToolRegistry;
use crate::utils;
use reqwest::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::time::{timeout, Duration as TokioDuration};

const SCOUT_LLM_TIMEOUT: TokioDuration = TokioDuration::from_secs(22);

/// Зависимости для Scout 2.0 LLM (тот же `call_llm`, что и в остальном CLI).
#[derive(Clone)]
pub struct ScoutLlm {
    pub http: Client,
    pub api_key: String, // legacy fallback
    pub key_provider: Arc<dyn crate::key_provider::KeyProvider>,
    pub config: Arc<AEGISConfig>,
    pub local: Option<LocalLlmClient>,
}

pub struct Scout {
    tools: Arc<ToolRegistry>,
    audit: Arc<AuditTrail>,
    air_gapped: bool,
    step_timeout: TokioDuration,
    llm: Option<ScoutLlm>,
}

impl Scout {
    pub fn new(tools: Arc<ToolRegistry>, audit: Arc<AuditTrail>, air_gapped: bool) -> Self {
        Self {
            tools,
            audit,
            air_gapped,
            step_timeout: TokioDuration::from_secs(12),
            llm: None,
        }
    }

    pub fn with_llm(mut self, llm: ScoutLlm) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Phase 1 базовый цикл + Scout 2.0 (LLM при доступности, иначе эвристика).
    pub async fn run_basic(&self, topic: &str) -> Result<Vec<KnowledgeItem>, String> {
        self.run_scout_pipeline(topic).await
    }

    pub async fn run_advanced(&self, topic: &str) -> Result<Vec<KnowledgeItem>, String> {
        // Расширенный режим = тот же пайплайн Scout 2.0 (доп. запросы можно добавить позже).
        self.run_scout_pipeline(topic).await
    }

    async fn run_scout_pipeline(&self, topic: &str) -> Result<Vec<KnowledgeItem>, String> {
        if self.air_gapped {
            return Err("Air-gapped mode: scout research cycle disabled".into());
        }

        let topic = topic.trim();
        if topic.is_empty() {
            return Err("topic is empty".into());
        }

        let _ = self.audit.log_event(
            "scout",
            &format!("start_pipeline topic={}", clip(topic, 160)),
            0.25,
            true,
        );

        let (osint_findings, darknet_findings, sources) = self.collect_dual(topic).await?;
        let now = chrono::Utc::now().timestamp();
        let mut findings: Vec<KnowledgeItem> = Vec::new();
        for f in &osint_findings {
            findings.push(self.make_item(topic, f, "white_sources", &sources, now));
        }
        for f in &darknet_findings {
            findings.push(self.make_item(topic, f, "darknet-heuristic", &sources, now));
        }

        // Scout 2.0: классификация + summary (LLM + fallback), затем гипотезы.
        if self.llm.is_some() && !self.air_gapped {
            let peer_digest: Vec<String> = findings
                .iter()
                .map(|x| {
                    format!(
                        "[{:?}|{}] {}",
                        x.item_type,
                        x.source,
                        clip(x.summary.as_deref().unwrap_or(&x.content), 220)
                    )
                })
                .collect();

            let n = findings.len();
            for idx in 0..n {
                if findings[idx].item_type == KnowledgeType::Hypothesis {
                    continue;
                }
                let peers: String = (0..n)
                    .filter(|&j| j != idx)
                    .filter_map(|j| peer_digest.get(j).cloned())
                    .take(8)
                    .collect::<Vec<_>>()
                    .join("\n");

                let it = &mut findings[idx];
                match self.classify_item_with_peers(it, topic, &peers).await {
                    Ok(t) => it.item_type = t,
                    Err(_) => {}
                }
                match self.enrich_summary(it).await {
                    Ok(s) if !s.trim().is_empty() => {
                        it.summary = Some(s);
                    }
                    _ => {}
                }
                self.refresh_item_meta_after_type_change(it);
            }

            match self.generate_hypotheses(&findings, topic).await {
                Ok(hyps) => {
                    findings.extend(hyps);
                }
                Err(e) => {
                    tracing::warn!(target: "scout", "generate_hypotheses failed (graceful): {}", e);
                    let _ = self.audit.log_event(
                        "scout",
                        &format!("scout_hypotheses_generated n=0 err={}", clip(&e, 120)),
                        0.2,
                        false,
                    );
                }
            }
        } else {
            // Нет LLM-контекста: только эвристические гипотезы (как раньше).
            for h in build_hypotheses(topic, &osint_findings, &darknet_findings) {
                let mut it = self.make_item(topic, &h, "internal", &sources, now);
                it.item_type = KnowledgeType::Hypothesis;
                it.confidence = 0.55;
                it.tags.push("hypothesis".into());
                findings.push(it);
            }
            let _ = self.audit.log_event(
                "scout",
                "scout_hypotheses_generated n=heuristic_only mode=no_llm",
                0.15,
                true,
            );
        }

        let _ = self.audit.log_event(
            "scout",
            &format!(
                "completed_pipeline topic={} items={}",
                clip(topic, 120),
                findings.len()
            ),
            0.25,
            true,
        );

        crate::metrics::record_scout_run(&findings);

        Ok(findings)
    }

    /// Генерирует 2–5 гипотез (KnowledgeType::Hypothesis) через LLM; при сбое — Ok(vec![]) или частичный разбор.
    pub async fn generate_hypotheses(
        &self,
        findings: &[KnowledgeItem],
        topic: &str,
    ) -> Result<Vec<KnowledgeItem>, String> {
        if self.llm.is_none() || self.air_gapped {
            return Ok(vec![]);
        }

        let findings_summary: String = findings
            .iter()
            .filter(|f| f.item_type != KnowledgeType::Hypothesis)
            .take(20)
            .map(|f| {
                format!(
                    "- {:?} | {} | conf={:.2}\n  summary: {}\n  excerpt: {}",
                    f.item_type,
                    f.source,
                    f.confidence,
                    clip(f.summary.as_deref().unwrap_or(""), 200),
                    clip(&f.content, 400)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let system = "You are an experienced Threat Intelligence analyst for AEGIS. \
You output ONLY valid JSON (no markdown, no commentary). \
Be factual and conservative; do not invent CVEs or incidents not supported by the input text.";

        let user = format!(
            r#"You are an experienced Threat Intelligence analyst.

Topic: "{}"

Collected findings:
{}

Based on these findings, produce 3 to 5 hypotheses about threats, trends, or important conclusions.

Return ONLY a JSON array. Each element must be an object with keys:
"summary" (string), "content" (string), "confidence" (number 0-1), "tags" (array of strings), "related_iocs" (array of strings).

Example shape: [{{"summary":"...","content":"...","confidence":0.7,"tags":["x"],"related_iocs":[]}}]"#,
            clip(topic, 200),
            if findings_summary.is_empty() {
                "(no structured findings)".to_string()
            } else {
                findings_summary
            }
        );

        let raw = match self.scout_llm_call(system, &user, false).await {
            Some(r) if r != "[BLOCKED]" && !r.trim().is_empty() => r,
            _ => {
                let _ = self.audit.log_event(
                    "scout",
                    "scout_hypotheses_generated n=0 avg_conf=0.000 reason=llm_empty_or_blocked",
                    0.2,
                    false,
                );
                return Ok(vec![]);
            }
        };

        let slice = extract_json_array_owned(&raw).ok_or_else(|| "hypotheses: no JSON array in LLM output".to_string())?;
        let arr: Vec<HypothesisJson> = serde_json::from_str(&slice)
            .map_err(|e| format!("hypotheses JSON parse: {}", e))?;

        let now = chrono::Utc::now().timestamp();
        let mut out = Vec::new();
        let mut conf_sum = 0.0f64;
        let mut n = 0usize;
        for h in arr.into_iter().take(5) {
            let conf = h.confidence.clamp(0.0, 1.0);
            conf_sum += conf;
            n += 1;
            let mut tags = h.tags.unwrap_or_default();
            if !tags.iter().any(|t| t == "hypothesis") {
                tags.push("hypothesis".into());
            }
            tags.push("scout_llm".into());
            let mut iocs = h.related_iocs.unwrap_or_default();
            merge_iocs(&mut iocs, extract_iocs(&h.content.to_lowercase()));
            merge_iocs(&mut iocs, extract_iocs(&h.summary.to_lowercase()));

            out.push(KnowledgeItem {
                id: uuid::Uuid::new_v4().to_string(),
                item_type: KnowledgeType::Hypothesis,
                content: format!("TOPIC: {}\n{}", clip(topic, 160), h.content),
                summary: Some(h.summary),
                source: "scout_llm_hypothesis".to_string(),
                confidence: conf,
                verified_by: vec!["scout".into(), "scout_llm".into()],
                tags,
                related_iocs: iocs,
                first_seen: now,
                last_seen: now,
                embedding_id: None,
                content_hash: String::new(),
                feedback: None,
            });
        }

        let avg = if n > 0 { conf_sum / n as f64 } else { 0.0 };
        let _ = self.audit.log_event(
            "scout",
            &format!(
                "scout_hypotheses_generated n={} avg_conf={:.3}",
                out.len(),
                avg
            ),
            0.25,
            true,
        );

        Ok(out)
    }

    /// Классификация White / Black через LLM; при confidence < 0.7 или сбое — эвристика из `make_item`-логики.
    pub async fn classify_item(&self, item: &KnowledgeItem) -> Result<KnowledgeType, String> {
        let (topic_guess, _) = split_topic_body(&item.content, "");
        let topic_line = topic_guess.trim();
        self.classify_item_with_peers(item, topic_line, "").await
    }

    /// Внутренний вызов с темой и сводкой «соседних» находок (контекст для LLM).
    pub(crate) async fn classify_item_with_peers(
        &self,
        item: &KnowledgeItem,
        topic: &str,
        peer_findings: &str,
    ) -> Result<KnowledgeType, String> {
        let (topic_guess, body) = split_topic_body(&item.content, topic);
        let heuristic = heuristic_item_type(&topic_guess, &body.to_lowercase(), &item.source);

        if self.llm.is_none() || self.air_gapped {
            let _ = self.audit.log_event(
                "scout",
                &format!(
                    "scout_item_classified mode=heuristic_only type={:?} conf=n/a id={}",
                    heuristic,
                    clip(&item.id, 12)
                ),
                0.15,
                true,
            );
            return Ok(heuristic);
        }

        let system = "You are AEGIS Threat Intelligence classifier. Output ONLY valid JSON, no markdown. \
Classify each finding as White (legitimate knowledge, remediation, vendor guidance) or Black (threat, exploitation, malicious activity).";

        let user = format!(
            r#"Decide if the finding is a threat/anomaly (Black) or legitimate/remediation knowledge (White).

Topic: {}
Related findings (short digest):
{}

Finding to classify:
SOURCE: {}
TYPE (hint): {:?}
CONTENT:
{}

Return JSON only:
{{"classification":"White"|"Black","confidence":0.0-1.0,"reason":"short explanation"}}"#,
            clip(&topic_guess, 200),
            if peer_findings.is_empty() {
                "(none)".to_string()
            } else {
                clip(peer_findings, 2500)
            },
            item.source,
            item.item_type,
            clip(&item.content, 3500)
        );

        let raw = match self.scout_llm_call(system, &user, false).await {
            Some(r) if r != "[BLOCKED]" && !r.trim().is_empty() => r,
            _ => {
                let _ = self.audit.log_event(
                    "scout",
                    &format!(
                        "scout_item_classified fallback=heuristic type={:?} id={}",
                        heuristic,
                        clip(&item.id, 12)
                    ),
                    0.2,
                    false,
                );
                return Ok(heuristic);
            }
        };

        let slice = match extract_json_object_owned(&raw) {
            Some(s) => s,
            None => {
                let _ = self.audit.log_event(
                    "scout",
                    "scout_item_classified fallback=heuristic reason=bad_json",
                    0.2,
                    false,
                );
                return Ok(heuristic);
            }
        };

        let parsed: ClassifyJson = match serde_json::from_str(&slice) {
            Ok(v) => v,
            Err(_) => {
                let _ = self.audit.log_event(
                    "scout",
                    "scout_item_classified fallback=heuristic reason=parse",
                    0.2,
                    false,
                );
                return Ok(heuristic);
            }
        };

        let conf = parsed.confidence.clamp(0.0, 1.0);
        let chosen = if conf < 0.7 {
            let _ = self.audit.log_event(
                "scout",
                &format!(
                    "scout_item_classified fallback=heuristic llm_conf={:.2} type={:?} id={}",
                    conf,
                    heuristic,
                    clip(&item.id, 12)
                ),
                0.18,
                true,
            );
            heuristic
        } else {
            match parsed.classification.to_lowercase().as_str() {
                "white" => KnowledgeType::White,
                "black" => KnowledgeType::Black,
                _ => heuristic,
            }
        };

        let _ = self.audit.log_event(
            "scout",
            &format!(
                "scout_item_classified type={:?} conf={:.2} reason={} id={}",
                chosen,
                conf,
                clip(&parsed.reason, 160),
                clip(&item.id, 12)
            ),
            0.22,
            true,
        );

        Ok(chosen)
    }

    /// Краткое информативное summary (1–2 предложения); при ошибке — исходное.
    pub async fn enrich_summary(&self, item: &KnowledgeItem) -> Result<String, String> {
        let baseline = item
            .summary
            .clone()
            .unwrap_or_else(|| clip(&item.content, 240));

        if self.llm.is_none() || self.air_gapped {
            let _ = self
                .audit
                .log_event("scout", "scout_summary_enriched mode=skipped_no_llm", 0.1, true);
            return Ok(baseline);
        }

        let system = "You are AEGIS CTI editor. Output ONLY valid JSON with a single key \"summary\" (string). \
1-2 sentences, factual, no speculation beyond the text. No markdown.";

        let user = format!(
            "Rewrite into a clear 1-2 sentence summary highlighting key facts (vendor, CVE if any, risk/remediation).\n\nTEXT:\n{}",
            clip(&item.content, 4000)
        );

        let raw = match self.scout_llm_call(system, &user, false).await {
            Some(r) if r != "[BLOCKED]" && !r.trim().is_empty() => r,
            _ => {
                let _ = self.audit.log_event(
                    "scout",
                    "scout_summary_enriched mode=fallback_llm_unavailable",
                    0.15,
                    false,
                );
                return Ok(baseline);
            }
        };

        let slice = extract_json_object_owned(&raw).ok_or_else(|| "summary: no JSON object".to_string())?;
        #[derive(Deserialize)]
        struct S {
            summary: String,
        }
        let s: S = serde_json::from_str(&slice).map_err(|e| format!("summary JSON: {}", e))?;
        let out = s.summary.trim().to_string();
        if out.is_empty() {
            let _ = self
                .audit
                .log_event("scout", "scout_summary_enriched mode=fallback_empty", 0.12, false);
            return Ok(baseline);
        }

        let _ = self.audit.log_event(
            "scout",
            &format!(
                "scout_summary_enriched id={} len={}",
                clip(&item.id, 12),
                out.len()
            ),
            0.15,
            true,
        );

        Ok(out)
    }

    async fn scout_llm_call(&self, system: &str, user: &str, is_critical: bool) -> Option<String> {
        let deps = self.llm.as_ref()?;
        let fut = crate::call_llm(
            &deps.http,
            deps.key_provider.as_ref(),
            system,
            user,
            deps.config.as_ref(),
            deps.local.as_ref(),
            is_critical,
        );
        timeout(SCOUT_LLM_TIMEOUT, fut).await.ok().flatten()
    }

    fn refresh_item_meta_after_type_change(&self, it: &mut KnowledgeItem) {
        // Не накачиваем confidence принудительно: если LLM ошибся в классификации,
        // inflate confidence создаёт ложную уверенность в hallucinated данных.
        // Вместо этого: аккуратно обновляем теги и verified_by.
        // Confidence остаётся как есть или слегка снижается при Hypothesis.
        let original_conf = it.confidence;
        it.confidence = match it.item_type {
            // Для Hypothesis — небольшой penalty (данные ещё не верифицированы)
            KnowledgeType::Hypothesis => (original_conf * 0.95).clamp(0.1, 1.0),
            // Для остальных — без принудительного boost
            _ => original_conf,
        };

        if !it.tags.iter().any(|t| t == "scout_llm") {
            it.tags.push("scout_llm".into());
        }
        if !it.verified_by.iter().any(|v| v == "scout_llm") {
            it.verified_by.push("scout_llm".into());
        }
        match it.item_type {
            KnowledgeType::Black => {
                if !it.tags.iter().any(|t| t == "threat") {
                    it.tags.retain(|t| t != "remediation");
                    it.tags.push("threat".into());
                }
            }
            KnowledgeType::White => {
                if !it.tags.iter().any(|t| t == "remediation") {
                    it.tags.retain(|t| t != "threat");
                    it.tags.push("remediation".into());
                }
            }
            _ => {}
        }
    }

    fn make_item(&self, topic: &str, content: &str, source: &str, _sources: &[String], now: i64) -> KnowledgeItem {
        let lc_full = content.to_lowercase();
        let topic_lc = topic.to_lowercase();
        let lc = lc_full.replace(&topic_lc, " ");
        let item_type = heuristic_item_type(topic, &lc, source);

        let confidence = match item_type {
            KnowledgeType::Black => 0.78,
            KnowledgeType::White => 0.70,
            KnowledgeType::Hypothesis => 0.55,
            KnowledgeType::TTP => 0.60,
        };
        let mut tags = vec!["scout".to_string()];
        match item_type {
            KnowledgeType::Black => tags.push("threat".into()),
            KnowledgeType::White => tags.push("remediation".into()),
            KnowledgeType::Hypothesis => tags.push("hypothesis".into()),
            KnowledgeType::TTP => tags.push("ttp".into()),
        }
        KnowledgeItem {
            id: uuid::Uuid::new_v4().to_string(),
            item_type,
            content: format!("TOPIC: {}\n{}", clip(topic, 160), clip(content, 2000)),
            summary: Some(clip(content, 240)),
            source: source.to_string(),
            confidence,
            verified_by: vec!["scout".to_string()],
            tags,
            related_iocs: extract_iocs(&lc_full),
            first_seen: now,
            last_seen: now,
            embedding_id: None,
            content_hash: String::new(),
            feedback: None,
        }
    }

    async fn collect_dual(&self, topic: &str) -> Result<(Vec<String>, Vec<String>, Vec<String>), String> {
        let osint_queries = [
            format!("NVD CVE exploited in the wild {}", topic),
            format!("CISA KEV {}", topic),
            format!("CISA advisory {}", topic),
            format!("vendor security advisory {}", topic),
            format!("patch released {}", topic),
        ];
        let dn_queries = [
            format!("ransomware claim {} {}", topic, "forum post"),
            format!("initial access broker {} {}", topic, "sale"),
            format!("exploit PoC released {}", topic),
            format!("credential leak {} {}", topic, "paste"),
        ];

        let (osint, dn) = tokio::join!(
            self.run_queries("white_sources", &osint_queries),
            self.run_queries("darknet_heuristic", &dn_queries)
        );
        let (osint_findings, osint_sources) = osint?;
        let (darknet_findings, dn_sources) = dn?;

        let mut sources = Vec::new();
        sources.extend(osint_sources);
        sources.extend(dn_sources);
        sources.sort();
        sources.dedup();

        Ok((osint_findings, darknet_findings, sources))
    }

    async fn run_queries(&self, label: &str, queries: &[String]) -> Result<(Vec<String>, Vec<String>), String> {
        let mut findings = Vec::new();
        let mut sources = HashSet::new();
        for q in queries {
            if let Some(out) = self.web_search(q).await? {
                let ol = out.to_lowercase();
                if ol.contains("duckduckgo") || ol.contains("api.duckduckgo.com") {
                    sources.insert("duckduckgo".to_string());
                }
                findings.push(format!("[{}] {}", label, out));
            }
        }
        Ok((dedup_keep_order(findings), sources.into_iter().collect()))
    }

    async fn web_search(&self, query: &str) -> Result<Option<String>, String> {
        let mut params = HashMap::new();
        params.insert("query".to_string(), query.to_string());

        let fut = self.tools.execute("web_search", params);
        let res = timeout(self.step_timeout, fut)
            .await
            .map_err(|_| "web_search timeout".to_string())?;

        match res {
            Some(r) if r.success => Ok(Some(format!("{} -> {}", clip(query, 140), clip(&r.output, 700)))),
            Some(r) => Ok(Some(format!("{} -> [error] {}", clip(query, 140), r.error.unwrap_or_default()))),
            None => Err("web_search tool not available".into()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ClassifyJson {
    classification: String,
    confidence: f64,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct HypothesisJson {
    summary: String,
    content: String,
    confidence: f64,
    tags: Option<Vec<String>>,
    related_iocs: Option<Vec<String>>,
}

fn heuristic_item_type(_topic: &str, lc: &str, source: &str) -> KnowledgeType {
    let black_needles = [
        "exploited in the wild",
        "actively exploited",
        "kev",
        "proof of concept",
        "poc",
        "ransomware",
        "initial access",
        "broker",
        "credential leak",
        "leak",
        "attack",
        "weaponized",
    ];
    let white_needles = [
        "vendor advisory",
        "security advisory",
        "patch released",
        "mitigation",
        "workaround",
        "fixed in",
        "release notes",
        "upgrade",
    ];

    let mut item_type = if black_needles.iter().any(|n| lc.contains(n)) {
        KnowledgeType::Black
    } else if white_needles.iter().any(|n| lc.contains(n)) {
        KnowledgeType::White
    } else if source == "darknet-heuristic" {
        KnowledgeType::Black
    } else {
        KnowledgeType::White
    };

    if item_type == KnowledgeType::Black
        && lc.contains("cve")
        && white_needles.iter().any(|n| lc.contains(n))
        && !black_needles.iter().any(|n| lc.contains(n))
    {
        item_type = KnowledgeType::White;
    }

    item_type
}

fn split_topic_body(content: &str, fallback_topic: &str) -> (String, String) {
    if let Some(rest) = content.strip_prefix("TOPIC:") {
        if let Some(idx) = rest.find('\n') {
            let t = rest[..idx].trim().to_string();
            let body = rest[idx + 1..].to_string();
            return (if t.is_empty() { fallback_topic.to_string() } else { t }, body);
        }
        return (fallback_topic.to_string(), rest.trim().to_string());
    }
    (fallback_topic.to_string(), content.to_string())
}

fn extract_json_object_owned(text: &str) -> Option<String> {
    utils::extract_json_object_owned(text)
}

fn extract_json_array_owned(text: &str) -> Option<String> {
    utils::extract_json_array_owned(text)
}

#[allow(dead_code)]
fn strip_markdown_fence(text: &str) -> String {
    utils::strip_markdown_fence(text)
}

fn merge_iocs(dst: &mut Vec<String>, src: Vec<String>) {
    for v in src {
        if !dst.iter().any(|x| x == &v) {
            dst.push(v);
        }
    }
}

fn dedup_keep_order(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for it in items {
        if seen.insert(it.clone()) {
            out.push(it);
        }
    }
    out
}

fn build_hypotheses(topic: &str, osint: &[String], darknet: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!(
        "Hypothesis: '{}' is associated with active exploitation attempts; prioritize patching + detection content.",
        clip(topic, 120)
    ));
    if osint.iter().any(|s| s.to_lowercase().contains("kev")) {
        out.push("Hypothesis: CISA KEV signal present → treat as high priority for enterprise exposure reduction.".into());
    }
    if darknet.iter().any(|s| s.to_lowercase().contains("poc")) {
        out.push("Hypothesis: PoC/public exploit signal present → increase WAF/IDS signatures and shorten MTTP.".into());
    }
    out
}

fn clip(s: &str, max: usize) -> String {
    utils::clip(s, max)
}

fn extract_iocs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let s = text.replace(['\n', '\r', '\t', ',', ';', '(', ')', '[', ']', '{', '}', '"', '\''], " ");
    for tok in s.split_whitespace() {
        let t = tok.trim().trim_end_matches('.').trim_end_matches(':');
        if t.is_empty() {
            continue;
        }
        if t.len() >= 11 && t.to_uppercase().starts_with("CVE-") {
            let u = t.to_uppercase();
            let parts: Vec<&str> = u.split('-').collect();
            if parts.len() >= 3
                && parts[1].len() == 4
                && parts[1].chars().all(|c| c.is_ascii_digit())
                && parts[2].chars().all(|c| c.is_ascii_digit())
            {
                push_dedup(&mut out, u);
                continue;
            }
        }
        if t.chars().filter(|&c| c == '.').count() == 3 {
            if t.parse::<std::net::IpAddr>().is_ok() {
                push_dedup(&mut out, t.to_string());
                continue;
            }
        }
        let hex_len = t.len();
        if (hex_len == 32 || hex_len == 40 || hex_len == 64) && t.chars().all(|c| c.is_ascii_hexdigit()) {
            push_dedup(&mut out, t.to_lowercase());
            continue;
        }
        if t.contains('.') && !t.contains('/') && !t.contains('@') && t.len() <= 253 {
            let parts: Vec<&str> = t.split('.').collect();
            if parts.len() >= 2
                && parts.iter().all(|p| !p.is_empty() && p.len() <= 63)
                && parts.last().unwrap().len() >= 2
                && parts.last().unwrap().chars().all(|c| c.is_ascii_alphabetic())
            {
                push_dedup(&mut out, t.to_lowercase());
            }
        }
    }
    out
}

fn push_dedup(out: &mut Vec<String>, v: String) {
    if !out.iter().any(|x| x == &v) {
        out.push(v);
    }
}
