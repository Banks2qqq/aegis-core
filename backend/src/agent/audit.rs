//! Immutable audit trail with append-only log and hash chain.
//!
//! Each record includes `prev_hash` and `hash = SHA256(prev_hash || canonical_json)`.
//! This provides tamper-evidence for pilots (not a full KMS-backed WORM solution).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unix timestamp (milliseconds)
    pub timestamp_ms: i64,
    pub action: String,
    pub actor: String,
    pub risk: f64,
    pub human_approved: bool,
    pub prev_hash: String,
    pub hash: String,
}

pub struct AuditTrail {
    enabled: bool,
    immutable: bool,
    log_path: String,
    last_hash: Mutex<String>,
    // Буфер для отложенной записи (анти-IO bottleneck)
    buffer: Arc<Mutex<Vec<String>>>,
}

impl AuditTrail {
    pub fn new(enabled: bool, log_path: &str, immutable: bool) -> Self {
        Self {
            enabled,
            immutable,
            log_path: log_path.to_string(),
            last_hash: Mutex::new("GENESIS".to_string()),
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn init(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if let Some(parent) = Path::new(&self.log_path).parent() {
            fs::create_dir_all(parent).map_err(|e| format!("audit create_dir_all: {}", e))?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| format!("audit open: {}", e))?;

        // === Hash chain restoration ===
        // При каждом рестарте восстанавливаем last_hash из последней записи лога.
        // Без этого цепочка tamper-evidence ломается при каждом перезапуске.
        if let Ok(content) = fs::read_to_string(&self.log_path) {
            if let Some(last_line) = content.lines().rev().find(|l| !l.trim().is_empty()) {
                if let Ok(event) = serde_json::from_str::<AuditEvent>(last_line) {
                    match self.last_hash.lock() {
                        Ok(mut h) => {
                            *h = event.hash.clone();
                            tracing::info!(
                                target: "audit",
                                "Hash chain restored from log: tail_hash={}…",
                                &event.hash[..16.min(event.hash.len())]
                            );
                        }
                        Err(_) => {
                            tracing::warn!(target: "audit", "Could not lock last_hash during chain restore");
                        }
                    }
                } else {
                    tracing::warn!(
                        target: "audit",
                        "Last audit log line is not a valid AuditEvent; chain starts from GENESIS"
                    );
                }
            }
        }

        // Фоновый флашер буфера (каждые 5 секунд)
        let buffer = self.buffer.clone();
        let path = self.log_path.clone();
        let immutable = self.immutable;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                ticker.tick().await;
                let mut buf = buffer.lock().unwrap_or_else(|p| p.into_inner());
                if !buf.is_empty() {
                    if let Ok(mut file) = OpenOptions::new().append(true).open(&path) {
                        for line in buf.drain(..) {
                            let _ = file.write_all(line.as_bytes()).and_then(|_| file.write_all(b"\n"));
                        }
                        if immutable {
                            let _ = file.sync_data();
                        }
                    }
                }
            }
        });

        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn log_event(
        &self,
        actor: &str,
        action: &str,
        risk: f64,
        human_approved: bool,
    ) -> Result<AuditEvent, String> {
        if !self.enabled {
            return Ok(AuditEvent {
                timestamp_ms: chrono::Utc::now().timestamp_millis(),
                action: action.to_string(),
                actor: actor.to_string(),
                risk,
                human_approved,
                prev_hash: "DISABLED".to_string(),
                hash: "DISABLED".to_string(),
            });
        }

        // Canonical payload (no hash fields)
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        let mut prev = self
            .last_hash
            .lock()
            .map_err(|_| "audit last_hash mutex poisoned".to_string())?;
        let prev_hash = prev.clone();

        #[derive(Serialize)]
        struct Canonical<'a> {
            timestamp_ms: i64,
            action: &'a str,
            actor: &'a str,
            risk: f64,
            human_approved: bool,
        }

        let canonical = Canonical {
            timestamp_ms,
            action,
            actor,
            risk,
            human_approved,
        };

        let canonical_json =
            serde_json::to_string(&canonical).map_err(|e| format!("audit serialize: {}", e))?;

        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(canonical_json.as_bytes());
        let hash = hex::encode(hasher.finalize());

        let event = AuditEvent {
            timestamp_ms,
            action: action.to_string(),
            actor: actor.to_string(),
            risk,
            human_approved,
            prev_hash,
            hash: hash.clone(),
        };

        let line = serde_json::to_string(&event).map_err(|e| format!("audit serialize: {}", e))?;

        // Буферизированная запись вместо немедленного fsync
        {
            let mut buf = self.buffer.lock().map_err(|_| "audit buffer poisoned".to_string())?;
            buf.push(line.clone());
            // Сбрасываем на диск каждые 50 событий или при immutable-режиме
            if buf.len() >= 50 || self.immutable {
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.log_path)
                    .map_err(|e| format!("audit open: {}", e))?;
                for l in buf.drain(..) {
                    file.write_all(l.as_bytes())
                        .and_then(|_| file.write_all(b"\n"))
                        .map_err(|e| format!("audit write: {}", e))?;
                }
                if self.immutable {
                    let _ = file.sync_data();
                }
            }
        }

        *prev = hash;
        Ok(event)
    }

    pub fn read_last_lines(&self, n: usize) -> Result<Vec<String>, String> {
        if !self.enabled {
            return Ok(vec![]);
        }
        let content = fs::read_to_string(&self.log_path)
            .map_err(|e| format!("audit read: {}", e))?;
        let mut lines: Vec<&str> = content.lines().collect();
        if n == 0 || lines.is_empty() {
            return Ok(vec![]);
        }
        if lines.len() > n {
            lines = lines.split_off(lines.len() - n);
        }
        Ok(lines.into_iter().map(|s| s.to_string()).collect())
    }
}

