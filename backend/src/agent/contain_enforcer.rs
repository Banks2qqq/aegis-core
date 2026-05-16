//! PR4.2 — Persist containment + optional host enforcement (marker + iptables chain).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::isolation::{IsolationConfig, IsolationLevel, NetworkPolicy};

const IPTABLES_CHAIN: &str = "AEGIS_CONTAIN";

#[derive(Debug, Clone, Serialize)]
pub struct ContainRecord {
    pub cluster_id: String,
    pub severity: f64,
    pub isolation_level: String,
    pub runtime: String,
    pub network: String,
    pub enforced_at: i64,
    pub enforcement_mode: String,
    pub host_enforced: bool,
    pub iptables_rule: Option<String>,
}

pub struct ContainEnforcer {
    root: PathBuf,
}

impl ContainEnforcer {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        Self {
            root: data_root.as_ref().join("contain"),
        }
    }

    fn record_path(&self, cluster_id: &str) -> PathBuf {
        self.root.join(format!("{}.json", sanitize_id(cluster_id)))
    }

    pub fn network_label(policy: &NetworkPolicy) -> &'static str {
        match policy {
            NetworkPolicy::Full => "full",
            NetworkPolicy::OutboundOnly => "outbound_only",
            NetworkPolicy::Isolated => "isolated",
            NetworkPolicy::None => "none",
        }
    }

    pub fn enforce(
        &self,
        cluster_id: &str,
        severity: f64,
        iso: &IsolationConfig,
    ) -> Result<ContainRecord, String> {
        fs::create_dir_all(&self.root).map_err(|e| format!("mkdir contain: {}", e))?;

        let (host_enforced, enforcement_mode, iptables_rule) = if contain_enforce_enabled() {
            self.apply_host_enforcement(cluster_id, severity, iso)
        } else {
            (false, "policy_record".to_string(), None)
        };

        let record = ContainRecord {
            cluster_id: cluster_id.to_string(),
            severity,
            isolation_level: format!("{:?}", iso.level),
            runtime: iso.runtime.clone(),
            network: Self::network_label(&iso.network).to_string(),
            enforced_at: chrono::Utc::now().timestamp(),
            enforcement_mode,
            host_enforced,
            iptables_rule,
        };

        let path = self.record_path(cluster_id);
        let raw = serde_json::to_string_pretty(&record).map_err(|e| format!("json: {}", e))?;
        fs::write(path, raw).map_err(|e| format!("write contain record: {}", e))?;

        Ok(record)
    }

    fn apply_host_enforcement(
        &self,
        cluster_id: &str,
        severity: f64,
        iso: &IsolationConfig,
    ) -> (bool, String, Option<String>) {
        let marker_ok = Self::write_marker(cluster_id, iso);
        if let Some(rule) = Self::try_iptables_rule(cluster_id, severity, iso) {
            return (true, "iptables".into(), Some(rule));
        }
        if marker_ok {
            return (true, "host_marker".into(), None);
        }
        (false, "policy_record".into(), None)
    }

    fn write_marker(cluster_id: &str, iso: &IsolationConfig) -> bool {
        let marker_dir = std::env::var("AEGIS_CONTAIN_DIR")
            .unwrap_or_else(|_| "/opt/aegis/backend/data/contain/active".to_string());
        if fs::create_dir_all(&marker_dir).is_err() {
            return false;
        }
        let safe = sanitize_id(cluster_id);
        let body = format!(
            "cluster={}\nisolation={:?}\nruntime={}\nnetwork={}\n",
            cluster_id,
            iso.level,
            iso.runtime,
            Self::network_label(&iso.network)
        );
        fs::write(
            format!("{}/{}.marker", marker_dir.trim_end_matches('/'), safe),
            body,
        )
        .is_ok()
    }

    /// Custom chain + LOG rule (safe). Optional DROP when AEGIS_CONTAIN_DROP=1 and severity >= 0.9.
    fn try_iptables_rule(
        cluster_id: &str,
        severity: f64,
        iso: &IsolationConfig,
    ) -> Option<String> {
        if Command::new("iptables").arg("--version").output().is_err() {
            return None;
        }

        let _ = Command::new("iptables").args(["-N", IPTABLES_CHAIN]).status();

        if contain_hook_output() {
            let check = Command::new("iptables")
                .args(["-C", "OUTPUT", "-j", IPTABLES_CHAIN])
                .status();
            if check.map(|s| !s.success()).unwrap_or(true) {
                let _ = Command::new("iptables")
                    .args(["-I", "OUTPUT", "1", "-j", IPTABLES_CHAIN])
                    .status();
            }
        }

        let safe = sanitize_id(cluster_id);
        let comment = format!("aegis:{}", safe);
        let log_prefix = format!("AEGIS_CONTAIN[{}]: ", safe);

        let log_rule: Vec<&str> = vec![
            "-A",
            IPTABLES_CHAIN,
            "-m",
            "comment",
            "--comment",
            comment.as_str(),
            "-j",
            "LOG",
            "--log-prefix",
            log_prefix.as_str(),
        ];
        if Command::new("iptables").args(&log_rule).status().ok()?.success() {
            let mut desc = format!("LOG chain={} comment={}", IPTABLES_CHAIN, comment);

            if contain_drop_enabled()
                && severity >= 0.9
                && matches!(iso.level, IsolationLevel::High | IsolationLevel::Critical)
            {
                let drop_comment = format!("{}:drop", comment);
                let drop_args: Vec<&str> = vec![
                    "-A",
                    IPTABLES_CHAIN,
                    "-m",
                    "comment",
                    "--comment",
                    drop_comment.as_str(),
                    "-j",
                    "DROP",
                ];
                if Command::new("iptables").args(&drop_args).status().ok()?.success() {
                    desc.push_str("; DROP appended (AEGIS_CONTAIN_DROP=1)");
                }
            }

            return Some(desc);
        }
        None
    }
}

fn sanitize_id(cluster_id: &str) -> String {
    cluster_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn contain_enforce_enabled() -> bool {
    env_flag("AEGIS_CONTAIN_ENFORCE")
}

pub fn contain_drop_enabled() -> bool {
    env_flag("AEGIS_CONTAIN_DROP")
}

fn contain_hook_output() -> bool {
    env_flag("AEGIS_CONTAIN_HOOK_OUTPUT")
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
