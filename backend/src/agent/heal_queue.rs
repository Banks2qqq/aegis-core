//! H3 — Human-in-the-loop queue for healing patches (post-sandbox, pre-apply).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::healing_orchestrator::PatchRisk;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingHealPatch {
    pub patch_id: String,
    pub content: String,
    pub risk: PatchRisk,
    pub verification_passed: bool,
    pub verification_severity: f64,
    pub sandbox_passed: bool,
    pub anomaly_summary: String,
    pub queued_at: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PendingStore {
    pub items: Vec<PendingHealPatch>,
}

pub struct HealQueue {
    root: PathBuf,
}

impl HealQueue {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        let root = data_root.as_ref().join("healing");
        Self { root }
    }

    fn store_path(&self) -> PathBuf {
        self.root.join("pending.json")
    }

    fn load(&self) -> Result<PendingStore, String> {
        let path = self.store_path();
        if !path.exists() {
            return Ok(PendingStore::default());
        }
        let raw = fs::read_to_string(&path).map_err(|e| format!("read pending: {}", e))?;
        serde_json::from_str(&raw).map_err(|e| format!("parse pending: {}", e))
    }

    fn save(&self, store: &PendingStore) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|e| format!("mkdir healing: {}", e))?;
        let raw = serde_json::to_string_pretty(store).map_err(|e| format!("json: {}", e))?;
        fs::write(self.store_path(), raw).map_err(|e| format!("write pending: {}", e))
    }

    pub fn list_pending(&self) -> Result<Vec<PendingHealPatch>, String> {
        Ok(self
            .load()?
            .items
            .into_iter()
            .filter(|i| i.status == "pending")
            .collect())
    }

    pub fn get(&self, patch_id: &str) -> Result<Option<PendingHealPatch>, String> {
        Ok(self
            .load()?
            .items
            .into_iter()
            .find(|i| i.patch_id == patch_id)
            .filter(|i| i.status == "pending"))
    }

    pub fn enqueue(&self, item: PendingHealPatch) -> Result<(), String> {
        let mut store = self.load()?;
        if store.items.iter().any(|i| i.patch_id == item.patch_id && i.status == "pending") {
            return Err(format!("patch already pending: {}", item.patch_id));
        }
        store.items.retain(|i| i.patch_id != item.patch_id);
        store.items.push(item);
        self.save(&store)
    }

    pub fn mark(&self, patch_id: &str, status: &str) -> Result<Option<PendingHealPatch>, String> {
        let mut store = self.load()?;
        let mut found = None;
        for item in &mut store.items {
            if item.patch_id == patch_id && item.status == "pending" {
                item.status = status.to_string();
                found = Some(item.clone());
                break;
            }
        }
        self.save(&store)?;
        Ok(found)
    }

    pub fn remove_pending(&self, patch_id: &str) -> Result<Option<PendingHealPatch>, String> {
        let mut store = self.load()?;
        let pos = store
            .items
            .iter()
            .position(|i| i.patch_id == patch_id && i.status == "pending");
        let Some(idx) = pos else {
            return Ok(None);
        };
        let item = store.items.remove(idx);
        self.save(&store)?;
        Ok(Some(item))
    }
}
