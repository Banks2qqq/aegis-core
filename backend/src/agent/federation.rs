//! Federation Layer — delta sync, health probes, ops plane (PR3.2).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use sha2::Digest;
use tokio::sync::Mutex;

use crate::audit::AuditTrail;
use crate::config::{FederationConfig, FederationPeerConfig};
use crate::federation_auth::FEDERATION_TOKEN_HEADER;
use crate::distributed_oracle::ConsensusLayer;
use crate::dna_engine::DnaEngine;
use crate::knowledge::KnowledgeBase;

const HEALTH_TIMEOUT_SECS: u64 = 5;

/// Per-peer runtime (health + last sync) — FederationSync state.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PeerRuntime {
    pub last_sync_at: Option<i64>,
    pub last_sync_count: usize,
    pub last_sync_duration_ms: Option<u64>,
    pub last_error: Option<String>,
    pub last_health_at: Option<i64>,
    pub online: bool,
    pub health_ok: bool,
    pub federation_ready: bool,
    pub latency_ms: Option<u64>,
    pub remote_merkle: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerHealth {
    pub id: String,
    /// Public HTTPS base (health / dashboard).
    pub url: String,
    /// Federation mTLS listener (typically :8443).
    pub federation_url: String,
    pub online: bool,
    pub health_ok: bool,
    pub federation_ready: bool,
    pub latency_ms: Option<u64>,
    pub remote_merkle: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,
    pub last_sync_at: Option<i64>,
    pub last_sync_count: usize,
    /// `online` | `degraded` | `offline`
    pub status: String,
    pub last_sync_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FederationHealthReport {
    pub local_node_id: String,
    pub local_public_url: Option<String>,
    pub local_federation_url: Option<String>,
    pub local_online: bool,
    pub local_merkle: String,
    pub peer_count: usize,
    pub peers_online: usize,
    pub peers: Vec<PeerHealth>,
    pub checked_at: i64,
    pub auth_enabled: bool,
    pub mtls_enabled: bool,
}

/// Ops metrics for dashboard + `/api/federation/metrics`.
#[derive(Debug, Clone, Serialize)]
pub struct FederationOpsMetrics {
    pub checked_at: i64,
    pub local_node_id: String,
    pub peers: Vec<PeerOpsMetric>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerOpsMetric {
    pub id: String,
    pub status: String,
    pub url: String,
    pub federation_url: String,
    pub federation_ready: bool,
    pub latency_ms: Option<u64>,
    pub last_sync_at: Option<i64>,
    pub last_sync_duration_ms: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncOutcome {
    pub peer_id: String,
    pub peer_url: String,
    pub synced: usize,
    pub success: bool,
    pub error: Option<String>,
    pub raft_index: Option<u64>,
    pub local_merkle_before: String,
    pub local_merkle_after: String,
    pub remote_merkle_before: Option<String>,
    pub remote_merkle_after: Option<String>,
    pub merkle_match: bool,
    pub auth_used: bool,
    pub merkle_repaired: usize,
}

pub struct FederationLayer {
    pub kb: Arc<KnowledgeBase>,
    #[allow(dead_code)]
    dna: Arc<DnaEngine>,
    audit: Arc<AuditTrail>,
    peers: Vec<FederationPeerConfig>,
    local_node_id: String,
    /// FederationSync — peer health + sync timestamps.
    sync_state: Arc<Mutex<HashMap<String, PeerRuntime>>>,
    raft: Option<Arc<Mutex<ConsensusLayer>>>,
    http: reqwest::Client,
    shared_secret: Option<String>,
    mtls_enabled: bool,
    local_public_url: Option<String>,
    local_federation_url: Option<String>,
}

impl FederationLayer {
    pub fn new(
        kb: Arc<KnowledgeBase>,
        dna: Arc<DnaEngine>,
        audit: Arc<AuditTrail>,
        federation_cfg: &FederationConfig,
        local_node_id: String,
    ) -> Self {
        let http = crate::federation_client::build_federation_http_client(federation_cfg);
        let shared_secret = federation_cfg.effective_shared_secret();
        let mtls_enabled = crate::federation_client::mtls_configured(federation_cfg);

        Self {
            kb,
            dna,
            audit,
            peers: federation_cfg.peers.clone(),
            local_node_id,
            sync_state: Arc::new(Mutex::new(HashMap::new())),
            raft: None,
            http,
            shared_secret,
            mtls_enabled,
            local_public_url: federation_cfg.public_url.clone(),
            local_federation_url: federation_cfg.federation_listen_url(),
        }
    }

    pub fn auth_enabled(&self) -> bool {
        self.shared_secret.is_some()
            || self.peers.iter().any(|p| {
                p.auth_token
                    .as_ref()
                    .map(|t| !t.trim().is_empty())
                    .unwrap_or(false)
            })
    }

    pub fn mtls_enabled(&self) -> bool {
        self.mtls_enabled
    }

    fn peer_token(&self, peer: Option<&FederationPeerConfig>) -> Option<String> {
        peer.and_then(|p| {
            p.auth_token
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .or_else(|| self.shared_secret.clone())
    }

    fn apply_peer_auth(
        &self,
        builder: reqwest::RequestBuilder,
        peer: Option<&FederationPeerConfig>,
    ) -> reqwest::RequestBuilder {
        if let Some(token) = self.peer_token(peer) {
            builder.header(FEDERATION_TOKEN_HEADER, token)
        } else {
            builder
        }
    }

    async fn fetch_remote_merkle(
        &self,
        base: &str,
        peer: Option<&FederationPeerConfig>,
    ) -> Option<String> {
        let url = format!("{}/federation/merkle", base.trim_end_matches('/'));
        match self
            .apply_peer_auth(self.http.get(&url), peer)
            .timeout(std::time::Duration::from_secs(HEALTH_TIMEOUT_SECS))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp.json::<serde_json::Value>().await.ok().and_then(
                |body| {
                    body.get("merkle_root")
                        .or_else(|| body.get("root"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                },
            ),
            _ => None,
        }
    }

    async fn my_content_hashes(&self) -> Result<Vec<String>, String> {
        Ok(self
            .kb
            .get_all_hashes()
            .await?
            .into_iter()
            .map(|(_, h)| h)
            .filter(|h| !h.is_empty())
            .collect())
    }

    async fn fetch_remote_hashes(
        &self,
        base: &str,
        peer: Option<&FederationPeerConfig>,
    ) -> Result<Vec<String>, String> {
        let url = format!("{}/federation/hashes", base.trim_end_matches('/'));
        let resp = self
            .apply_peer_auth(self.http.get(&url), peer)
            .timeout(std::time::Duration::from_secs(HEALTH_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| format!("hashes request failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("hashes HTTP {}", resp.status()));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("hashes parse: {}", e))?;
        if let Some(arr) = body.get("hashes").and_then(|v| v.as_array()) {
            return Ok(arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect());
        }
        Err("hashes: unexpected JSON shape".to_string())
    }

    async fn ingest_federated_batch(
        &self,
        items: Vec<crate::knowledge_item::KnowledgeItem>,
    ) -> usize {
        let mut n = 0usize;
        for item in items {
            if self.ingest_federated_item(item).await.is_ok() {
                n += 1;
            }
        }
        n
    }

    async fn pull_missing_from_peer(
        &self,
        base: &str,
        peer: Option<&FederationPeerConfig>,
    ) -> Result<usize, String> {
        let my_hashes = self.my_content_hashes().await?;
        let resp = self
            .apply_peer_auth(
                self.http
                    .post(format!("{}/federation/missing", base.trim_end_matches('/')))
                    .json(&my_hashes),
                peer,
            )
            .send()
            .await
            .map_err(|e| format!("missing pull failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("missing pull HTTP {}", resp.status()));
        }
        let items: Vec<crate::knowledge_item::KnowledgeItem> = resp
            .json()
            .await
            .map_err(|e| format!("missing pull parse: {}", e))?;
        Ok(self.ingest_federated_batch(items).await)
    }

    async fn push_missing_to_peer(
        &self,
        base: &str,
        peer: Option<&FederationPeerConfig>,
    ) -> Result<usize, String> {
        let remote_hashes = self.fetch_remote_hashes(base, peer).await?;
        let remote_set: std::collections::HashSet<String> = remote_hashes.into_iter().collect();
        let mut to_push = Vec::new();
        for (_, hash) in self.kb.get_all_hashes().await? {
            if hash.is_empty() || remote_set.contains(&hash) {
                continue;
            }
            if let Ok(Some(item)) = self.kb.get_by_content_hash(&hash).await {
                to_push.push(item);
            }
        }
        if to_push.is_empty() {
            return Ok(0);
        }
        let count = to_push.len();
        let resp = self
            .apply_peer_auth(
                self.http
                    .post(format!("{}/federation/receive", base.trim_end_matches('/')))
                    .json(&to_push),
                peer,
            )
            .send()
            .await
            .map_err(|e| format!("receive push failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("receive push HTTP {}", resp.status()));
        }
        Ok(count)
    }

    async fn repair_merkle_with_peer(
        &self,
        base: &str,
        peer: Option<&FederationPeerConfig>,
    ) -> Result<usize, String> {
        let pulled = self.pull_missing_from_peer(base, peer).await?;
        let pushed = self.push_missing_to_peer(base, peer).await?;
        Ok(pulled + pushed)
    }

    pub fn with_raft(mut self, raft: Arc<Mutex<ConsensusLayer>>) -> Self {
        self.raft = Some(raft);
        self
    }

    pub fn configured_peers(&self) -> &[FederationPeerConfig] {
        &self.peers
    }

    pub fn local_node_id(&self) -> &str {
        &self.local_node_id
    }

    fn peer_by_id(&self, id: &str) -> Option<&FederationPeerConfig> {
        self.peers.iter().find(|p| p.id == id)
    }

    fn peer_by_url(&self, url: &str) -> Option<&FederationPeerConfig> {
        let norm = url.trim_end_matches('/');
        self.peers.iter().find(|p| {
            p.health_base() == norm || p.federation_base() == norm
        })
    }

    fn resolve_peer(&self, peer_url: &str) -> (String, String) {
        if let Some(p) = self.peer_by_url(peer_url) {
            return (p.id.clone(), p.federation_base());
        }
        (
            format!("peer-{}", &peer_url.chars().take(24).collect::<String>()),
            peer_url.to_string(),
        )
    }

    async fn runtime_for(&self, peer_id: &str) -> PeerRuntime {
        self.sync_state
            .lock()
            .await
            .get(peer_id)
            .cloned()
            .unwrap_or_default()
    }

    async fn set_runtime(&self, peer_id: &str, rt: PeerRuntime) {
        self.sync_state
            .lock()
            .await
            .insert(peer_id.to_string(), rt);
    }

    async fn record_sync_success(&self, peer_id: &str, synced: usize, duration_ms: u64) {
        let mut rt = self.runtime_for(peer_id).await;
        let now = chrono::Utc::now().timestamp();
        rt.last_sync_at = Some(now);
        rt.last_sync_count = synced;
        rt.last_sync_duration_ms = Some(duration_ms);
        rt.last_error = None;
        self.set_runtime(peer_id, rt).await;
        crate::metrics::federation_sync(peer_id, true, duration_ms);
        crate::metrics::federation_mtls_outbound(peer_id, "success");
    }

    async fn record_sync_error(&self, peer_id: &str, err: &str, duration_ms: u64) {
        let mut rt = self.runtime_for(peer_id).await;
        rt.last_error = Some(err.to_string());
        rt.last_sync_duration_ms = Some(duration_ms);
        self.set_runtime(peer_id, rt).await;
        crate::metrics::federation_sync(peer_id, false, duration_ms);
        let outbound = if err.contains("403") || err.contains("mTLS") || err.contains("certificate") {
            "mtls_rejected"
        } else {
            "error"
        };
        crate::metrics::federation_mtls_outbound(peer_id, outbound);
    }

    async fn replicate_sync_to_raft(&self, peer_id: &str, synced: usize) -> Option<u64> {
        let Some(raft) = &self.raft else {
            return None;
        };
        let cmd = format!("federation_sync|peer={}|synced={}", peer_id, synced);
        let mut guard = raft.lock().await;
        match guard.append_and_replicate(&cmd).await {
            Ok(idx) => Some(idx),
            Err(e) => {
                tracing::warn!("Federation sync: raft replicate failed: {}", e);
                None
            }
        }
    }

    /// Возвращает Merkle Root текущего состояния знаний.
    pub async fn get_merkle_root(&self) -> Result<String, String> {
        let hashes = self
            .kb
            .get_all_hashes()
            .await
            .map_err(|e| format!("Failed to get hashes: {}", e))?;

        if hashes.is_empty() {
            return Ok("merkle_empty".to_string());
        }

        let mut leaf_hashes: Vec<String> = Vec::new();
        for (id, content_hash) in hashes {
            let leaf = if !content_hash.is_empty() {
                content_hash
            } else {
                format!("{:x}", sha2::Sha256::digest(id.as_bytes()))
            };
            leaf_hashes.push(leaf);
        }
        leaf_hashes.sort();
        let combined = leaf_hashes.join("");
        Ok(format!(
            "merkle_{:x}",
            sha2::Sha256::digest(combined.as_bytes())
        ))
    }

    /// Probe `/health` + optional `/federation/merkle` on a configured peer.
    pub async fn probe_peer(&self, peer: &FederationPeerConfig) -> PeerHealth {
        let health_base = peer.health_base();
        let fed_base = peer.federation_base();
        let health_url = format!("{}/health", health_base);
        let merkle_url = format!("{}/federation/merkle", fed_base);
        let started = Instant::now();

        let mut online = false;
        let mut health_ok = false;
        let mut federation_ready = false;
        let mut remote_merkle = None;
        let mut version = None;
        let mut error = None;

        let peer_ref = Some(peer);
        match self
            .apply_peer_auth(self.http.get(&health_url), peer_ref)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                online = true;
                health_ok = true;
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    version = body
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
            Ok(resp) => {
                online = true;
                error = Some(format!("health HTTP {}", resp.status()));
            }
            Err(e) => {
                error = Some(format!("health unreachable: {}", e));
            }
        }

        if online {
            if let Some(root) = self.fetch_remote_merkle(&fed_base, peer_ref).await {
                remote_merkle = Some(root);
                federation_ready = true;
            } else if let Ok(resp) = self
                .apply_peer_auth(self.http.get(&merkle_url), peer_ref)
                .send()
                .await
            {
                federation_ready = false;
                if error.is_none() {
                    error = Some(format!(
                        "federation merkle HTTP {} (auth token or mTLS may be required)",
                        resp.status()
                    ));
                }
            } else if error.is_none() {
                error = Some("federation merkle unreachable".to_string());
            }
        }

        let latency_ms = Some(started.elapsed().as_millis() as u64);
        let now = chrono::Utc::now().timestamp();

        let mut runtime = self.runtime_for(&peer.id).await;
        runtime.last_health_at = Some(now);
        runtime.online = online && health_ok;
        runtime.health_ok = health_ok;
        runtime.federation_ready = federation_ready;
        runtime.latency_ms = latency_ms;
        runtime.remote_merkle = remote_merkle.clone();
        let status = crate::metrics::federation_peer_status_label(
            runtime.online,
            health_ok,
            federation_ready,
        )
        .to_string();
        runtime.status = status.clone();
        if let Some(ms) = latency_ms {
            crate::metrics::federation_peer_latency_ms(&peer.id, ms);
        }
        crate::metrics::federation_peer_status(&peer.id, &status);
        self.set_runtime(&peer.id, runtime.clone()).await;

        PeerHealth {
            id: peer.id.clone(),
            url: peer.url.clone(),
            federation_url: peer.federation_base(),
            online: runtime.online,
            health_ok,
            federation_ready,
            latency_ms,
            remote_merkle,
            version,
            error,
            last_sync_at: runtime.last_sync_at,
            last_sync_count: runtime.last_sync_count,
            status,
            last_sync_duration_ms: runtime.last_sync_duration_ms,
        }
    }

    pub async fn ops_metrics(&self) -> FederationOpsMetrics {
        let mut peers = Vec::new();
        for p in &self.peers {
            if p.id == self.local_node_id {
                continue;
            }
            let rt = self.runtime_for(&p.id).await;
            peers.push(PeerOpsMetric {
                id: p.id.clone(),
                status: if rt.status.is_empty() {
                    crate::metrics::federation_peer_status_label(
                        rt.online,
                        rt.health_ok,
                        rt.federation_ready,
                    )
                    .to_string()
                } else {
                    rt.status.clone()
                },
                url: p.url.clone(),
                federation_url: p.federation_base(),
                federation_ready: rt.federation_ready,
                latency_ms: rt.latency_ms,
                last_sync_at: rt.last_sync_at,
                last_sync_duration_ms: rt.last_sync_duration_ms,
                last_error: rt.last_error.clone(),
            });
        }
        FederationOpsMetrics {
            checked_at: chrono::Utc::now().timestamp(),
            local_node_id: self.local_node_id.clone(),
            peers,
        }
    }

    /// Full health report for `/api/federation/health`.
    pub async fn health_report(&self) -> FederationHealthReport {
        let local_merkle = self.get_merkle_root().await.unwrap_or_else(|e| {
            format!("merkle_error:{}", e)
        });
        let mut peers = Vec::new();
        for p in &self.peers {
            if p.id == self.local_node_id {
                continue;
            }
            peers.push(self.probe_peer(p).await);
        }
        let peers_online = peers.iter().filter(|p| p.online).count();
        FederationHealthReport {
            local_node_id: self.local_node_id.clone(),
            local_public_url: self.local_public_url.clone(),
            local_federation_url: self.local_federation_url.clone(),
            local_online: true,
            local_merkle,
            peer_count: self.peers.len(),
            peers_online,
            peers,
            checked_at: chrono::Utc::now().timestamp(),
            auth_enabled: self.auth_enabled(),
            mtls_enabled: self.mtls_enabled(),
        }
    }

    /// PR5 — periodic delta sync for all configured peers.
    pub fn start_background_sync(self: Arc<Self>, interval_secs: u64) {
        if interval_secs == 0 {
            return;
        }
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                tick.tick().await;
                let results = self.sync_all_peers().await;
                let summary: Vec<_> = results
                    .iter()
                    .map(|r| format!("{}:synced={}:ok={}", r.peer_id, r.synced, r.success))
                    .collect();
                tracing::info!("Federation background sync: {}", summary.join(", "));
            }
        });
    }

    /// Background health monitor (updates sync_state without blocking API).
    pub fn start_health_monitor(self: Arc<Self>, interval_secs: u64) {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                tick.tick().await;
                for p in self.peers.clone() {
                    if p.id == self.local_node_id {
                        continue;
                    }
                    let _ = self.probe_peer(&p).await;
                }
            }
        });
    }

    /// Register configured federation peers in Raft (DistributedOracle cluster view).
    pub async fn register_peers_in_raft(&self) {
        let Some(raft) = &self.raft else {
            return;
        };
        let mut guard = raft.lock().await;
        guard.register_node(&self.local_node_id);
        for p in &self.peers {
            if p.id != self.local_node_id {
                guard.register_node(&p.id);
            }
        }
    }

    /// Delta sync with peer URL (from config or ad-hoc URL).
    pub async fn sync_with_peer(&self, peer_url: &str) -> Result<SyncOutcome, String> {
        let (peer_id, url) = self.resolve_peer(peer_url);
        self.sync_with_peer_resolved(&peer_id, &url).await
    }

    pub async fn sync_with_peer_id(&self, peer_id: &str) -> Result<SyncOutcome, String> {
        let url = self
            .peer_by_id(peer_id)
            .map(|p| p.federation_base())
            .ok_or_else(|| format!("Unknown peer_id: {}", peer_id))?;
        self.sync_with_peer_resolved(peer_id, &url).await
    }

    async fn sync_with_peer_resolved(
        &self,
        peer_id: &str,
        peer_url: &str,
    ) -> Result<SyncOutcome, String> {
        let sync_started = Instant::now();
        let base = peer_url.trim_end_matches('/');
        let peer_cfg = self.peer_by_id(peer_id);
        let auth_used = self.peer_token(peer_cfg).is_some();
        let local_merkle_before = self.get_merkle_root().await.unwrap_or_else(|e| {
            format!("merkle_error:{}", e)
        });
        let remote_merkle_before = self.fetch_remote_merkle(base, peer_cfg).await;
        let my_last_seen = self.kb.get_last_seen().await.unwrap_or(0);

        let changed_resp = match self
            .apply_peer_auth(
                self.http
                    .post(format!("{}/federation/changed_since", base))
                    .json(&my_last_seen),
                peer_cfg,
            )
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let ms = sync_started.elapsed().as_millis() as u64;
                let err = format!("Request failed: {}", e);
                self.record_sync_error(peer_id, &err, ms).await;
                return Err(err);
            }
        };

        let status = changed_resp.status();
        let raw_text = changed_resp
            .text()
            .await
            .map_err(|e| format!("Failed to read text: {}", e))?;

        if !status.is_success() {
            let err = format!("Peer returned {} (raw: {})", status, raw_text);
            let ms = sync_started.elapsed().as_millis() as u64;
            self.record_sync_error(peer_id, &err, ms).await;
            let local_merkle_after = self.get_merkle_root().await.unwrap_or_else(|e| {
                format!("merkle_error:{}", e)
            });
            return Ok(SyncOutcome {
                peer_id: peer_id.to_string(),
                peer_url: peer_url.to_string(),
                synced: 0,
                success: false,
                error: Some(err),
                raft_index: None,
                local_merkle_before,
                local_merkle_after,
                remote_merkle_before,
                remote_merkle_after: None,
                merkle_match: false,
                auth_used,
                merkle_repaired: 0,
            });
        }

        let changed: Vec<crate::knowledge_item::KnowledgeItem> =
            serde_json::from_str(&raw_text)
                .map_err(|e| format!("Parse failed: {} (raw: {})", e, raw_text))?;

        let mut synced = 0usize;
        for item in changed {
            match item.item_type {
                crate::knowledge_item::KnowledgeType::White => {
                    let _ = self.kb.ingest_white(item).await;
                }
                crate::knowledge_item::KnowledgeType::Black => {
                    let _ = self.kb.ingest_black(item).await;
                }
                _ => {}
            }
            synced += 1;
        }

        let sync_ms = sync_started.elapsed().as_millis() as u64;
        self.record_sync_success(peer_id, synced, sync_ms).await;
        let raft_index = self.replicate_sync_to_raft(peer_id, synced).await;
        let mut local_merkle_after = self.get_merkle_root().await.unwrap_or_else(|e| {
            format!("merkle_error:{}", e)
        });
        let mut remote_merkle_after = self.fetch_remote_merkle(base, peer_cfg).await;
        let mut merkle_match = remote_merkle_after
            .as_ref()
            .map(|r| r == &local_merkle_after)
            .unwrap_or(false);
        let mut merkle_repaired = 0usize;

        if !merkle_match {
            match self.repair_merkle_with_peer(base, peer_cfg).await {
                Ok(n) if n > 0 => {
                    merkle_repaired = n;
                    synced += n;
                    local_merkle_after = self.get_merkle_root().await.unwrap_or(local_merkle_after);
                    remote_merkle_after = self.fetch_remote_merkle(base, peer_cfg).await;
                    merkle_match = remote_merkle_after
                        .as_ref()
                        .map(|r| r == &local_merkle_after)
                        .unwrap_or(false);
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("Federation merkle repair peer={}: {}", peer_id, e),
            }
        }

        let _ = self.audit.log_event(
            "federation",
            &format!(
                "delta_sync_completed peer_id={} url={} synced={} repaired={} merkle_match={} raft_index={:?}",
                peer_id, peer_url, synced, merkle_repaired, merkle_match, raft_index
            ),
            0.3,
            true,
        );

        Ok(SyncOutcome {
            peer_id: peer_id.to_string(),
            peer_url: peer_url.to_string(),
            synced,
            success: true,
            error: None,
            raft_index,
            local_merkle_before,
            local_merkle_after,
            remote_merkle_before,
            remote_merkle_after,
            merkle_match,
            auth_used,
            merkle_repaired,
        })
    }

    /// Sync all peers from `config.yaml` `federation.peers`.
    pub async fn sync_all_peers(&self) -> Vec<SyncOutcome> {
        let mut results = Vec::new();
        for p in self.peers.clone() {
            if p.id == self.local_node_id {
                continue;
            }
            let fed_url = p.federation_base();
            let outcome = self
                .sync_with_peer_resolved(&p.id, &fed_url)
                .await
                .unwrap_or_else(|e| SyncOutcome {
                    peer_id: p.id.clone(),
                    peer_url: fed_url.clone(),
                    synced: 0,
                    success: false,
                    error: Some(e),
                    raft_index: None,
                    local_merkle_before: String::new(),
                    local_merkle_after: String::new(),
                    remote_merkle_before: None,
                    remote_merkle_after: None,
                    merkle_match: false,
                    auth_used: self.auth_enabled(),
                    merkle_repaired: 0,
                });
            results.push(outcome);
        }
        results
    }

    pub async fn ingest_federated_item(
        &self,
        item: crate::knowledge_item::KnowledgeItem,
    ) -> Result<(), String> {
        let existing = match item.item_type {
            crate::knowledge_item::KnowledgeType::White => self.kb.get_white_by_id(&item.id).await,
            crate::knowledge_item::KnowledgeType::Black => self.kb.get_black_by_id(&item.id).await,
            _ => None,
        };

        if let Some(existing) = existing {
            if item.last_seen <= existing.last_seen {
                return Ok(());
            }
        }

        match item.item_type {
            crate::knowledge_item::KnowledgeType::White => {
                let _ = self.kb.ingest_white(item).await?;
            }
            crate::knowledge_item::KnowledgeType::Black => {
                let _ = self.kb.ingest_black(item).await?;
            }
            _ => {}
        }
        Ok(())
    }

    fn format_last_sync(rt: &PeerRuntime) -> String {
        if let Some(ts) = rt.last_sync_at {
            if rt.last_sync_count > 0 {
                return format!(
                    "{} items @ {}",
                    rt.last_sync_count,
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| ts.to_string())
                );
            }
            return chrono::DateTime::from_timestamp(ts, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| ts.to_string());
        }
        if let Some(err) = &rt.last_error {
            return format!("error: {}", crate::utils::clip(err, 80));
        }
        if rt.online {
            return "reachable (no sync yet)".to_string();
        }
        "never".to_string()
    }

    /// Dashboard nodes from config + live health/sync state.
    pub async fn get_all_nodes(&self) -> Vec<serde_json::Value> {
        let local_merkle = self.get_merkle_root().await.ok();
        let mut out = vec![serde_json::json!({
            "id": self.local_node_id,
            "url": self.local_public_url.as_deref().unwrap_or("(local)"),
            "federation_url": self.local_federation_url,
            "online": true,
            "health_ok": true,
            "federation_ready": true,
            "last_sync": "local primary",
            "role": "primary",
            "merkle_root": local_merkle,
        })];

        for p in &self.peers {
            if p.id == self.local_node_id {
                continue;
            }
            let health = self.probe_peer(p).await;
            let rt = self.runtime_for(&p.id).await;
            let merkle_match = health
                .remote_merkle
                .as_ref()
                .zip(local_merkle.as_ref())
                .map(|(r, l)| r == l)
                .unwrap_or(false);
            out.push(serde_json::json!({
                "id": p.id,
                "url": p.url,
                "federation_url": health.federation_url,
                "status": health.status,
                "last_sync_duration_ms": health.last_sync_duration_ms,
                "online": health.online,
                "health_ok": health.health_ok,
                "federation_ready": health.federation_ready,
                "last_sync": Self::format_last_sync(&rt),
                "last_sync_at": rt.last_sync_at,
                "last_sync_count": rt.last_sync_count,
                "latency_ms": health.latency_ms,
                "remote_merkle": health.remote_merkle,
                "merkle_root": health.remote_merkle,
                "merkle_match": merkle_match,
                "version": health.version,
                "error": health.error,
            }));
        }
        out
    }
}
