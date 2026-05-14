//! DNA Engine 2.5 — взвешенные агрегаты, разделение White/Black, decay, миграция со старого `aegis_dna.json`.

use crate::audit::AuditTrail;
use crate::knowledge_item::{KnowledgeFeedback, KnowledgeItem, KnowledgeType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub const DNA_SNAPSHOT_VERSION: u32 = 25;

/// Множитель decay для записей reinforcement, не попавших в текущий батч.
const REINFORCEMENT_STALE_DECAY: f64 = 0.94;
/// Слияние при повторном усилении того же `id`.
const REINFORCEMENT_MERGE_ALPHA: f64 = 0.98;
const REINFORCEMENT_MAX_KEYS: usize = 512;
const CUMULATIVE_MAP_DECAY: f64 = 0.92;
const TOP_K: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReinforcementRecord {
    pub last_reinforced_at: i64,
    /// Накопленная «важность» по id (для decay между прогонами).
    pub importance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnaKnowledgeRef {
    pub id: String,
    pub item_type: KnowledgeType,
    pub source: String,
    pub confidence: f64,
    pub tags: Vec<String>,
    pub related_iocs: Vec<String>,
    #[serde(default)]
    pub verified_by: Vec<String>,
    #[serde(default)]
    pub weight: f64,
    #[serde(default)]
    pub last_reinforced_at: i64,
    #[serde(default)]
    pub feedback: Option<KnowledgeFeedback>,
}

impl DnaKnowledgeRef {
    pub fn from_item(item: &KnowledgeItem, weight: f64, now_ts: i64) -> Self {
        Self {
            id: item.id.clone(),
            item_type: item.item_type.clone(),
            source: item.source.clone(),
            confidence: item.confidence,
            tags: item.tags.clone(),
            related_iocs: item.related_iocs.clone(),
            verified_by: item.verified_by.clone(),
            weight,
            last_reinforced_at: now_ts,
            feedback: item.feedback,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnaUpdate {
    pub timestamp_ms: i64,
    pub topic: String,
    pub items: Vec<DnaKnowledgeRef>,
}

/// Старый nested aggregate (до 2.5) — только для десериализации миграции.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnaAggregate {
    pub total_items: usize,
    pub by_type: HashMap<String, usize>,
    pub by_source: HashMap<String, usize>,
    pub top_tags: Vec<(String, usize)>,
    pub top_iocs: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnaAggregateDiff {
    pub delta_total_items: i64,
    pub delta_by_type: HashMap<String, i64>,
    pub delta_by_source: HashMap<String, i64>,
}

/// Старый снимок (схема «D») для миграции.
#[derive(Debug, Clone, Deserialize)]
struct LegacyDnaSnapshot {
    timestamp_ms: i64,
    topic: String,
    items: Vec<DnaKnowledgeRef>,
    #[allow(dead_code)]
    aggregate: DnaAggregate,
    #[allow(dead_code)]
    diff_from_previous: Option<DnaAggregateDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnaDiff {
    pub delta_total_items: i64,
    pub delta_white_knowledge_count: i64,
    pub delta_black_knowledge_count: i64,
    pub delta_hypothesis_count: i64,
    pub delta_avg_confidence: f64,
    pub delta_total_weighted_importance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnaSnapshot {
    #[serde(default = "default_dna_version")]
    pub version: u32,
    #[serde(alias = "timestamp_ms")]
    pub timestamp: i64,
    pub topic: String,
    pub total_items: usize,
    pub white_knowledge_count: usize,
    pub black_knowledge_count: usize,
    pub hypothesis_count: usize,
    pub ttp_count: usize,
    /// Сумма весов текущего батча (качество × тип × верификация).
    pub total_weighted_importance: f64,
    pub avg_confidence: f64,
    pub weighted_avg_confidence: f64,
    pub top_tags: Vec<(String, f64)>,
    pub top_iocs: Vec<(String, f64)>,
    pub top_sources: Vec<(String, f64)>,
    pub top_white_tags: Vec<(String, f64)>,
    pub top_black_tags: Vec<(String, f64)>,
    pub white_iocs: Vec<(String, f64)>,
    pub black_iocs: Vec<(String, f64)>,
    pub items: Vec<DnaKnowledgeRef>,
    #[serde(default)]
    pub reinforcement: HashMap<String, ReinforcementRecord>,
    /// Накопительные взвешенные счётчики (после decay + вклад текущего батча).
    #[serde(default)]
    pub cumulative_white_tags: HashMap<String, f64>,
    #[serde(default)]
    pub cumulative_black_tags: HashMap<String, f64>,
    #[serde(default)]
    pub cumulative_white_iocs: HashMap<String, f64>,
    #[serde(default)]
    pub cumulative_black_iocs: HashMap<String, f64>,
    pub last_update_diff: Option<DnaDiff>,
}

pub struct DnaEngine {
    path: String,
    audit: Arc<AuditTrail>,
}

/// Вес элемента для DNA 2.5 (качество и доверие), с учётом human feedback.
pub fn calculate_weight(item: &KnowledgeItem) -> f64 {
    let fb = match &item.feedback {
        None => 1.0,
        Some(KnowledgeFeedback::Useful) => 1.15,
        Some(KnowledgeFeedback::NotUseful) => 0.55,
        Some(KnowledgeFeedback::FalsePositive) => 0.15,
        Some(KnowledgeFeedback::NeedsReview) => 0.85,
    };
    let base = item.confidence.clamp(0.0, 1.0);
    let verification_bonus = match item.verified_by.len() {
        0 => 0.0,
        1 => 0.1,
        2 => 0.2,
        _ => 0.3,
    };
    let human_bonus = if item.verified_by.iter().any(|v| v == "human") {
        0.3
    } else {
        0.0
    };
    let type_bonus = match item.item_type {
        KnowledgeType::Black => 0.15,
        KnowledgeType::White => 0.0,
        KnowledgeType::Hypothesis => -0.1,
        KnowledgeType::TTP => 0.0,
    };
    ((base + verification_bonus + human_bonus + type_bonus).clamp(0.1, 1.0) * fb).clamp(0.05, 1.25)
}

impl DnaEngine {
    pub fn new(path: &str, audit: Arc<AuditTrail>) -> Self {
        Self {
            path: path.to_string(),
            audit,
        }
    }

    /// Полное обновление по `KnowledgeItem` (Scout / ingest): веса, White/Black агрегаты, decay, diff.
    pub async fn update_with_items(&self, topic: &str, items: &[KnowledgeItem]) -> Result<DnaSnapshot, String> {
        let path = self.path.clone();
        let audit = self.audit.clone();
        let topic = topic.to_string();
        let topic_audit = topic.clone();
        let items_vec: Vec<KnowledgeItem> = items.to_vec();

        let snap = tokio::task::spawn_blocking(move || -> Result<DnaSnapshot, String> {
            let prev = load_any_snapshot(&path);
            let now = chrono::Utc::now().timestamp();
            let snapshot = build_snapshot_v25(&topic, &items_vec, now, prev.as_ref());
            let json = serde_json::to_string_pretty(&snapshot).map_err(|e| format!("dna serialize: {}", e))?;
            // === Атомарная запись: write-to-temp + rename ===
            // Предотвращает повреждение снимка при краше в середине записи.
            let tmp_path = format!("{}.tmp", path);
            std::fs::write(&tmp_path, &json).map_err(|e| format!("dna write tmp: {}", e))?;
            std::fs::rename(&tmp_path, &path).map_err(|e| format!("dna atomic rename: {}", e))?;
            Ok(snapshot)
        })
        .await
        .map_err(|e| format!("dna join: {}", e))??;

        let delta = snap
            .last_update_diff
            .as_ref()
            .map(|d| d.delta_total_items.max(0) as u64)
            .unwrap_or(snap.total_items as u64);
        crate::metrics::dna_snapshot_update(
            snap.white_knowledge_count,
            snap.black_knowledge_count,
            snap.hypothesis_count,
            snap.ttp_count,
            snap.avg_confidence,
            delta,
        );

        let _ = audit.log_event(
            "dna_engine",
            &format!(
                "dna_v25_updated topic={} items={} weighted={:.3} wavg_conf={:.3}",
                clip_topic(&topic_audit),
                snap.total_items,
                snap.total_weighted_importance,
                snap.weighted_avg_confidence
            ),
            0.2,
            true,
        );

        Ok(snap)
    }

    /// Обратная совместимость: старый вызов через `DnaUpdate` (без полного `KnowledgeItem`).
    pub async fn update(&self, update: DnaUpdate) -> Result<(), String> {
        let items: Vec<KnowledgeItem> = update
            .items
            .iter()
            .map(knowledge_item_from_ref)
            .collect();
        self.update_with_items(&update.topic, &items).await?;
        Ok(())
    }
}

fn clip_topic(t: &str) -> String {
    if t.len() <= 120 {
        t.to_string()
    } else {
        format!("{}…", &t[..120])
    }
}

fn knowledge_item_from_ref(r: &DnaKnowledgeRef) -> KnowledgeItem {
    let now = chrono::Utc::now().timestamp();
    KnowledgeItem {
        id: r.id.clone(),
        item_type: r.item_type.clone(),
        content: String::new(),
        summary: None,
        source: r.source.clone(),
        confidence: r.confidence,
        verified_by: if r.verified_by.is_empty() {
            vec![]
        } else {
            r.verified_by.clone()
        },
        tags: r.tags.clone(),
        related_iocs: r.related_iocs.clone(),
        first_seen: now,
        last_seen: now,
        embedding_id: None,
        content_hash: String::new(),
        feedback: r.feedback,
    }
}

fn default_dna_version() -> u32 {
    DNA_SNAPSHOT_VERSION
}

fn load_any_snapshot(path: &str) -> Option<DnaSnapshot> {
    let s = std::fs::read_to_string(path).ok()?;
    if let Ok(legacy) = serde_json::from_str::<LegacyDnaSnapshot>(&s) {
        return Some(migrate_legacy_to_v25(legacy));
    }
    serde_json::from_str::<DnaSnapshot>(&s).ok()
}

fn migrate_legacy_to_v25(legacy: LegacyDnaSnapshot) -> DnaSnapshot {
    let items: Vec<KnowledgeItem> = legacy.items.iter().map(knowledge_item_from_ref).collect();
    let now_sec = (legacy.timestamp_ms / 1000).max(0);
    build_snapshot_v25(&legacy.topic, &items, now_sec, None)
}

fn build_snapshot_v25(topic: &str, items: &[KnowledgeItem], now: i64, prev: Option<&DnaSnapshot>) -> DnaSnapshot {
    // Уникальные id в батче (dedup статистики по одному прогону).
    let mut seen_ids: HashMap<String, &KnowledgeItem> = HashMap::new();
    for it in items {
        seen_ids.entry(it.id.clone()).or_insert(it);
    }
    let unique: Vec<&KnowledgeItem> = seen_ids.into_values().collect();

    let mut total_w = 0.0_f64;
    let mut conf_sum = 0.0_f64;
    let mut w_conf_sum = 0.0_f64;
    let mut white_n = 0usize;
    let mut black_n = 0usize;
    let mut hyp_n = 0usize;
    let mut ttp_n = 0usize;

    let mut run_white_tags: HashMap<String, f64> = HashMap::new();
    let mut run_black_tags: HashMap<String, f64> = HashMap::new();
    let mut run_white_iocs: HashMap<String, f64> = HashMap::new();
    let mut run_black_iocs: HashMap<String, f64> = HashMap::new();
    let mut run_sources: HashMap<String, f64> = HashMap::new();
    let mut run_tags_all: HashMap<String, f64> = HashMap::new();
    let mut run_iocs_all: HashMap<String, f64> = HashMap::new();

    for it in &unique {
        let w = calculate_weight(it);
        total_w += w;
        conf_sum += it.confidence;
        w_conf_sum += it.confidence * w;
        match it.item_type {
            KnowledgeType::White => white_n += 1,
            KnowledgeType::Black => black_n += 1,
            KnowledgeType::Hypothesis => hyp_n += 1,
            KnowledgeType::TTP => ttp_n += 1,
        }
        *run_sources.entry(it.source.clone()).or_default() += w;
        for t in &it.tags {
            *run_tags_all.entry(t.clone()).or_default() += w;
        }
        for ioc in &it.related_iocs {
            *run_iocs_all.entry(ioc.clone()).or_default() += w;
        }
        match it.item_type {
            KnowledgeType::White => {
                for t in &it.tags {
                    *run_white_tags.entry(t.clone()).or_default() += w;
                }
                for ioc in &it.related_iocs {
                    *run_white_iocs.entry(ioc.clone()).or_default() += w;
                }
            }
            KnowledgeType::Black => {
                for t in &it.tags {
                    *run_black_tags.entry(t.clone()).or_default() += w;
                }
                for ioc in &it.related_iocs {
                    *run_black_iocs.entry(ioc.clone()).or_default() += w;
                }
            }
            _ => {}
        }
    }

    let n = unique.len().max(1);
    let avg_conf = conf_sum / n as f64;
    let weighted_avg_conf = if total_w > 0.0 {
        w_conf_sum / total_w
    } else {
        0.0
    };

    // Cumulative maps: decay then merge run contributions
    let mut cum_wt = prev
        .map(|p| p.cumulative_white_tags.clone())
        .unwrap_or_default();
    let mut cum_bt = prev
        .map(|p| p.cumulative_black_tags.clone())
        .unwrap_or_default();
    let mut cum_wi = prev
        .map(|p| p.cumulative_white_iocs.clone())
        .unwrap_or_default();
    let mut cum_bi = prev
        .map(|p| p.cumulative_black_iocs.clone())
        .unwrap_or_default();

    decay_map(&mut cum_wt);
    decay_map(&mut cum_bt);
    decay_map(&mut cum_wi);
    decay_map(&mut cum_bi);

    merge_map(&mut cum_wt, &run_white_tags);
    merge_map(&mut cum_bt, &run_black_tags);
    merge_map(&mut cum_wi, &run_white_iocs);
    merge_map(&mut cum_bi, &run_black_iocs);

    prune_small_entries(&mut cum_wt, 200);
    prune_small_entries(&mut cum_bt, 200);
    prune_small_entries(&mut cum_wi, 200);
    prune_small_entries(&mut cum_bi, 200);

    let incoming_ids: std::collections::HashSet<String> =
        unique.iter().map(|i| i.id.clone()).collect();

    // Reinforcement per id
    let mut reinforcement = prev
        .map(|p| p.reinforcement.clone())
        .unwrap_or_default();
    for (k, r) in reinforcement.iter_mut() {
        if !incoming_ids.contains(k) {
            r.importance *= REINFORCEMENT_STALE_DECAY;
        }
    }
    reinforcement.retain(|k, r| incoming_ids.contains(k) || r.importance > 0.01);

    for it in &unique {
        let w = calculate_weight(it);
        let e = reinforcement.entry(it.id.clone()).or_insert(ReinforcementRecord {
            last_reinforced_at: now,
            importance: 0.0,
        });
        e.importance = e.importance * REINFORCEMENT_MERGE_ALPHA + w;
        e.importance = e.importance.min(50.0);
        e.last_reinforced_at = now;
    }
    if reinforcement.len() > REINFORCEMENT_MAX_KEYS {
        let mut pairs: Vec<(String, ReinforcementRecord)> = reinforcement.drain().collect();
        pairs.sort_by(|a, b| {
            b.1.importance
                .partial_cmp(&a.1.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        pairs.truncate(REINFORCEMENT_MAX_KEYS);
        reinforcement = pairs.into_iter().collect();
    }

    let top_white_tags = top_from_map(&cum_wt, TOP_K);
    let top_black_tags = top_from_map(&cum_bt, TOP_K);
    let white_iocs = top_from_map(&cum_wi, TOP_K);
    let black_iocs = top_from_map(&cum_bi, TOP_K);
    let top_tags = top_from_map(&run_tags_all, TOP_K);
    let top_iocs = top_from_map(&run_iocs_all, TOP_K);
    let top_sources = top_from_map(&run_sources, TOP_K);

    let stored_refs: Vec<DnaKnowledgeRef> = unique
        .iter()
        .take(100)
        .map(|it| DnaKnowledgeRef::from_item(it, calculate_weight(it), now))
        .collect();

    let mut snap = DnaSnapshot {
        version: DNA_SNAPSHOT_VERSION,
        timestamp: now,
        topic: topic.to_string(),
        total_items: unique.len(),
        white_knowledge_count: white_n,
        black_knowledge_count: black_n,
        hypothesis_count: hyp_n,
        ttp_count: ttp_n,
        total_weighted_importance: total_w,
        avg_confidence: avg_conf,
        weighted_avg_confidence: weighted_avg_conf,
        top_tags,
        top_iocs,
        top_sources,
        top_white_tags,
        top_black_tags,
        white_iocs,
        black_iocs,
        items: stored_refs,
        reinforcement,
        cumulative_white_tags: cum_wt,
        cumulative_black_tags: cum_bt,
        cumulative_white_iocs: cum_wi,
        cumulative_black_iocs: cum_bi,
        last_update_diff: None,
    };

    if let Some(p) = prev {
        snap.last_update_diff = Some(DnaDiff {
            delta_total_items: snap.total_items as i64 - p.total_items as i64,
            delta_white_knowledge_count: snap.white_knowledge_count as i64
                - p.white_knowledge_count as i64,
            delta_black_knowledge_count: snap.black_knowledge_count as i64
                - p.black_knowledge_count as i64,
            delta_hypothesis_count: snap.hypothesis_count as i64 - p.hypothesis_count as i64,
            delta_avg_confidence: snap.avg_confidence - p.avg_confidence,
            delta_total_weighted_importance: snap.total_weighted_importance
                - p.total_weighted_importance,
        });
    }

    snap
}

fn decay_map(m: &mut HashMap<String, f64>) {
    m.retain(|_, v| {
        *v *= CUMULATIVE_MAP_DECAY;
        *v > 1e-6
    });
}

fn merge_map(dst: &mut HashMap<String, f64>, add: &HashMap<String, f64>) {
    for (k, v) in add {
        *dst.entry(k.clone()).or_default() += *v;
    }
}

fn prune_small_entries(m: &mut HashMap<String, f64>, keep: usize) {
    if m.len() <= keep {
        return;
    }
    let mut v: Vec<_> = m.iter().map(|(k, s)| (k.clone(), *s)).collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    v.truncate(keep);
    *m = v.into_iter().collect();
}

fn top_from_map(m: &HashMap<String, f64>, k: usize) -> Vec<(String, f64)> {
    let mut v: Vec<_> = m.iter().map(|(a, b)| (a.clone(), *b)).collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    v.truncate(k);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_human_black_higher_than_scout_white() {
        let now = 0i64;
        let mut black = KnowledgeItem {
            id: "1".into(),
            item_type: KnowledgeType::Black,
            content: "".into(),
            summary: None,
            source: "s".into(),
            confidence: 0.8,
            verified_by: vec!["human".into(), "critic".into(), "inquisitor".into()],
            tags: vec![],
            related_iocs: vec![],
            first_seen: now,
            last_seen: now,
            embedding_id: None,
            content_hash: String::new(),
            feedback: None,
        };
        let w_strong = calculate_weight(&black);
        black.verified_by = vec!["scout".into()];
        black.item_type = KnowledgeType::White;
        let w_weak = calculate_weight(&black);
        assert!(w_strong > w_weak);
    }
}
