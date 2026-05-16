//! PR4.1 — Persist healing patches to disk with rollback snapshots (safe Config apply).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedPatchRecord {
    pub patch_id: String,
    pub path: String,
    pub applied_at: i64,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealingManifest {
    pub applied: Vec<AppliedPatchRecord>,
    pub snapshots: Vec<String>,
}

pub struct PatchApplier {
    root: PathBuf,
}

impl PatchApplier {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        let root = data_root.as_ref().join("healing");
        Self { root }
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    fn patches_dir(&self) -> PathBuf {
        self.root.join("applied")
    }

    fn snapshots_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }

    fn load_manifest(&self) -> Result<HealingManifest, String> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(HealingManifest {
                applied: vec![],
                snapshots: vec![],
            });
        }
        let raw = fs::read_to_string(&path).map_err(|e| format!("read manifest: {}", e))?;
        serde_json::from_str(&raw).map_err(|e| format!("parse manifest: {}", e))
    }

    fn save_manifest(&self, manifest: &HealingManifest) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|e| format!("mkdir healing: {}", e))?;
        let raw = serde_json::to_string_pretty(manifest).map_err(|e| format!("json: {}", e))?;
        fs::write(self.manifest_path(), raw).map_err(|e| format!("write manifest: {}", e))
    }

    /// Snapshot current manifest before apply (rollback anchor).
    pub fn prepare_snapshot(&self) -> Result<String, String> {
        fs::create_dir_all(self.snapshots_dir()).map_err(|e| format!("mkdir snapshots: {}", e))?;
        let id = uuid::Uuid::new_v4().to_string();
        let manifest = self.load_manifest()?;
        let snap_path = self.snapshots_dir().join(format!("{}.json", id));
        let raw = serde_json::to_string_pretty(&manifest).map_err(|e| format!("json: {}", e))?;
        fs::write(&snap_path, raw).map_err(|e| format!("write snapshot: {}", e))?;
        let mut m = manifest;
        m.snapshots.push(id.clone());
        if m.snapshots.len() > 32 {
            let drop_id = m.snapshots.remove(0);
            let _ = fs::remove_file(self.snapshots_dir().join(format!("{}.json", drop_id)));
        }
        self.save_manifest(&m)?;
        Ok(id)
    }

    /// Write patch file; returns path when `enforce` is true, else dry-run record only.
    pub fn apply_config_patch(
        &self,
        patch_id: &str,
        content: &str,
        enforce: bool,
    ) -> Result<AppliedPatchRecord, String> {
        fs::create_dir_all(self.patches_dir()).map_err(|e| format!("mkdir applied: {}", e))?;
        let now = chrono::Utc::now().timestamp();
        let file_name = format!("{}.patch", patch_id);
        let path = self.patches_dir().join(&file_name);
        let mode = if enforce { "applied" } else { "dry_run" };

        if enforce {
            fs::write(&path, content).map_err(|e| format!("write patch: {}", e))?;
        }

        let record = AppliedPatchRecord {
            patch_id: patch_id.to_string(),
            path: path.to_string_lossy().to_string(),
            applied_at: now,
            mode: mode.to_string(),
        };

        let mut manifest = self.load_manifest()?;
        manifest.applied.push(record.clone());
        self.save_manifest(&manifest)?;

        Ok(record)
    }

    pub fn rollback(&self, snapshot_id: &str) -> Result<(), String> {
        let snap_path = self.snapshots_dir().join(format!("{}.json", snapshot_id));
        if !snap_path.exists() {
            return Err(format!("snapshot not found: {}", snapshot_id));
        }
        let raw = fs::read_to_string(&snap_path).map_err(|e| format!("read snapshot: {}", e))?;
        let manifest: HealingManifest =
            serde_json::from_str(&raw).map_err(|e| format!("parse snapshot: {}", e))?;
        self.save_manifest(&manifest)?;
        tracing::warn!("PatchApplier: rolled back to snapshot {}", snapshot_id);
        Ok(())
    }

    pub fn applied_count(&self) -> Result<usize, String> {
        Ok(self.load_manifest()?.applied.len())
    }
}

pub fn heal_apply_enforced() -> bool {
    std::env::var("AEGIS_HEAL_APPLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
