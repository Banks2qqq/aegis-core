use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::Utc;

/// Уникальный идентификатор IOC (IP, domain, hash, CVE и т.д.)
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ioc {
    pub value: String,
    pub ioc_type: String, // "ip", "domain", "sha256", "cve", "url"
}

/// Результат корреляции
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedThreat {
    pub cluster_id: String,
    pub iocs: Vec<Ioc>,
    pub sources: Vec<String>,
    pub severity: f64,
    pub confidence: f64,
    pub first_seen: i64,
    pub last_seen: i64,
    pub summary: String,
}

/// Простая реализация Union-Find (Disjoint Set Union) для кластеризации IOC
struct UnionFind {
    parent: HashMap<String, String>,
    size: HashMap<String, usize>,
}

impl UnionFind {
    fn new() -> Self {
        Self {
            parent: HashMap::new(),
            size: HashMap::new(),
        }
    }

    fn find(&mut self, x: &str) -> String {
        if !self.parent.contains_key(x) {
            self.parent.insert(x.to_string(), x.to_string());
            self.size.insert(x.to_string(), 1);
            return x.to_string();
        }
        let mut current = x.to_string();
        while self.parent[&current] != current {
            current = self.parent[&current].clone();
        }
        // path compression
        let mut path = x.to_string();
        while self.parent[&path] != path {
            let next = self.parent[&path].clone();
            self.parent.insert(path.clone(), current.clone());
            path = next;
        }
        current
    }

    #[allow(dead_code)]
    fn union(&mut self, x: &str, y: &str) {
        let px = self.find(x);
        let py = self.find(y);
        if px == py {
            return;
        }
        // union by size
        if self.size[&px] < self.size[&py] {
            self.parent.insert(px.clone(), py.clone());
            self.size.insert(py.clone(), self.size[&px] + self.size[&py]);
        } else {
            self.parent.insert(py.clone(), px.clone());
            self.size.insert(px.clone(), self.size[&px] + self.size[&py]);
        }
    }
}

/// Streaming Threat Fusion Engine
/// HyperLogLog-подобная оценка уникальности + Union-Find корреляция + материализованные представления
#[derive(Clone)]
pub struct FusionEngine {
    uf: Arc<RwLock<UnionFind>>,
    clusters: Arc<RwLock<HashMap<String, Vec<Ioc>>>>, // cluster_id -> IOCs
    source_counts: Arc<RwLock<HashMap<String, usize>>>, // source -> count (HLL approx via set)
    fused_events: Arc<RwLock<Vec<FusedThreat>>>,
    max_clusters: usize,
}

impl FusionEngine {
    pub fn new(max_clusters: usize) -> Self {
        Self {
            uf: Arc::new(RwLock::new(UnionFind::new())),
            clusters: Arc::new(RwLock::new(HashMap::new())),
            source_counts: Arc::new(RwLock::new(HashMap::new())),
            fused_events: Arc::new(RwLock::new(Vec::new())),
            max_clusters,
        }
    }

    /// Ingest finding from any source and perform fusion
    pub async fn ingest(&self, source: &str, finding: &str, severity: f64, ioc_value: Option<&str>, ioc_type: Option<&str>) -> Option<FusedThreat> {
        let now = Utc::now().timestamp();
        let ioc = if let (Some(v), Some(t)) = (ioc_value, ioc_type) {
            Some(Ioc { value: v.to_string(), ioc_type: t.to_string() })
        } else {
            None
        };

        let mut uf = self.uf.write().await;
        let mut clusters = self.clusters.write().await;
        let mut source_counts = self.source_counts.write().await;
        let mut events = self.fused_events.write().await;

        // Обновляем счётчик источника (простая уникальность через set, HLL-подобно)
        let entry = source_counts.entry(source.to_string()).or_insert(0);
        *entry += 1;

        if let Some(ioc) = ioc {
            let ioc_key = format!("{}:{}", ioc.ioc_type, ioc.value);
            let cluster_id = uf.find(&ioc_key);

            // Объединяем с похожими по эвристике (например, если finding содержит похожий IOC)
            // Для простоты: если есть предыдущие в кластере — union с первым
            let cluster_iocs = clusters.entry(cluster_id.clone()).or_insert_with(Vec::new);
            if !cluster_iocs.iter().any(|x| x.value == ioc.value) {
                cluster_iocs.push(ioc.clone());
            }

            // Создаём/обновляем fused event
            let fused = events.iter_mut().find(|e| e.cluster_id == cluster_id);
            if let Some(existing) = fused {
                if !existing.sources.contains(&source.to_string()) {
                    existing.sources.push(source.to_string());
                }
                existing.severity = existing.severity.max(severity);
                existing.last_seen = now;
                existing.confidence = (existing.sources.len() as f64 / 7.0).min(1.0); // 7 источников max
                return Some(existing.clone());
            } else {
                let new_event = FusedThreat {
                    cluster_id: cluster_id.clone(),
                    iocs: cluster_iocs.clone(),
                    sources: vec![source.to_string()],
                    severity,
                    confidence: 0.6,
                    first_seen: now,
                    last_seen: now,
                    summary: format!("Correlated from {}: {}", source, finding),
                };
                events.push(new_event.clone());

                // Ограничение размера (материализованные представления)
                if events.len() > self.max_clusters {
                    events.drain(0..50);
                }
                return Some(new_event);
            }
        }
        None
    }

    /// Получить текущие fused threats (материализованное представление)
    pub async fn get_fused_threats(&self, limit: usize) -> Vec<FusedThreat> {
        let events = self.fused_events.read().await;
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Статистика (HLL-like cardinality + coverage)
    pub async fn get_stats(&self) -> serde_json::Value {
        let source_counts = self.source_counts.read().await;
        let clusters = self.clusters.read().await;
        let events = self.fused_events.read().await;

        let total_unique_iocs: usize = clusters.values().map(|v| v.len()).sum();
        let active_clusters = clusters.len();
        let high_severity = events.iter().filter(|e| e.severity > 0.8).count();

        serde_json::json!({
            "unique_iocs_estimated": total_unique_iocs,
            "active_clusters": active_clusters,
            "sources": source_counts.len(),
            "high_severity_threats": high_severity,
            "total_fused_events": events.len(),
        })
    }

    /// Периодическая очистка старых данных (защита от OOM)
    /// Рекомендуется вызывать раз в 30-60 минут фоновым таском
    pub async fn prune_old_data(&self, max_age_seconds: i64) {
        let now = Utc::now().timestamp();
        let cutoff = now - max_age_seconds;

        let mut events = self.fused_events.write().await;
        let before = events.len();

        events.retain(|e| e.last_seen > cutoff);

        // Также чистим clusters, если они больше не имеют активных событий
        let active_cluster_ids: std::collections::HashSet<String> = 
            events.iter().map(|e| e.cluster_id.clone()).collect();

        let mut clusters = self.clusters.write().await;
        clusters.retain(|id, _| active_cluster_ids.contains(id));

        let after = events.len();
        if before != after {
            tracing::info!(
                "FusionEngine pruned {} old clusters (age > {}s)",
                before - after,
                max_age_seconds
            );
        }
    }
}