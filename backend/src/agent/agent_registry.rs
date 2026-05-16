//! Operational agent registry (PR3.1) — live status for ops plane / dashboard.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::threat_hunter::ThreatHunter;

/// Context passed from HTTP layer (avoids circular dependency with `server`).
#[derive(Clone, Default)]
pub struct AgentDashboardContext {
    pub black_kb: usize,
    pub fusion_clusters: usize,
    pub honeypots: usize,
    pub healing_ready: bool,
    pub react_ready: bool,
    pub last_scout: Option<ScoutRunMeta>,
    pub last_react: Option<ReactRunMeta>,
}

#[derive(Clone)]
pub struct ScoutRunMeta {
    pub completed_at: i64,
    pub found: usize,
    pub fusion_updated: usize,
    pub healing_attempted: usize,
    pub healing_applied: usize,
    pub status: String,
}

#[derive(Clone)]
pub struct ReactRunMeta {
    pub mission: String,
    pub success: bool,
    pub iterations_used: u32,
    pub completed_at: i64,
}

pub const SCOUT_ID: &str = "scout";
pub const THREAT_HUNTER_ID: &str = "threat-hunter";
pub const HEALER_ID: &str = "healer";
pub const MTD_ID: &str = "mtd";
pub const REACT_ID: &str = "react";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentOperationalStatus {
    Active,
    Running,
    Paused,
    Error,
    Standby,
}

impl AgentOperationalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Error => "error",
            Self::Standby => "standby",
        }
    }
}

#[derive(Clone)]
struct AgentRuntime {
    name: String,
    role: String,
    enabled: bool,
    status: AgentOperationalStatus,
    current_task: String,
    last_run_at: Option<i64>,
    last_success_at: Option<i64>,
    last_error: Option<String>,
    run_count: u64,
    success_count: u64,
    error_count: u64,
    load: u8,
}

impl AgentRuntime {
    fn new(name: &str, role: &str, task: &str) -> Self {
        Self {
            name: name.into(),
            role: role.into(),
            enabled: true,
            status: AgentOperationalStatus::Standby,
            current_task: task.into(),
            last_run_at: None,
            last_success_at: None,
            last_error: None,
            run_count: 0,
            success_count: 0,
            error_count: 0,
            load: 0,
        }
    }
}

/// Shared registry for Scout, ThreatHunter, Healer, MTD (+ ReAct snapshot).
pub struct AgentRegistry {
    agents: Mutex<HashMap<String, AgentRuntime>>,
    hunter: Mutex<Option<Arc<ThreatHunter>>>,
    hunter_enabled: Arc<AtomicBool>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(
            SCOUT_ID.into(),
            AgentRuntime::new(
                "SCOUT",
                "FSTEC BDU PIPELINE",
                "Standby — run SCOUT from dashboard",
            ),
        );
        map.insert(
            THREAT_HUNTER_ID.into(),
            AgentRuntime::new(
                "THREAT HUNTER",
                "OSINT HUNT",
                "Background hunt cycle (7 sources)",
            ),
        );
        map.insert(
            HEALER_ID.into(),
            AgentRuntime::new(
                "HEALER",
                "SELF-HEALING",
                "Standby — runs on critical BDU during SCOUT",
            ),
        );
        map.insert(
            MTD_ID.into(),
            AgentRuntime::new(
                "MTD",
                "MOVING TARGET DEFENSE",
                "Background surface mutation",
            ),
        );

        Self {
            agents: Mutex::new(map),
            hunter: Mutex::new(None),
            hunter_enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn hunter_enabled_flag(&self) -> Arc<AtomicBool> {
        self.hunter_enabled.clone()
    }

    pub async fn attach_hunter(&self, hunter: Arc<ThreatHunter>) {
        *self.hunter.lock().await = Some(hunter);
        self.set_ready(THREAT_HUNTER_ID, "Background hunt scheduled").await;
    }

    pub async fn set_ready(&self, id: &str, task: &str) {
        let mut agents = self.agents.lock().await;
        if let Some(a) = agents.get_mut(id) {
            if a.enabled {
                a.status = AgentOperationalStatus::Active;
            }
            a.current_task = task.into();
        }
    }

    pub async fn mark_running(&self, id: &str, task: &str) {
        let now = chrono::Utc::now().timestamp();
        let mut agents = self.agents.lock().await;
        if let Some(a) = agents.get_mut(id) {
            if !a.enabled {
                return;
            }
            a.status = AgentOperationalStatus::Running;
            a.current_task = task.into();
            a.last_run_at = Some(now);
            a.run_count = a.run_count.saturating_add(1);
            a.load = a.load.saturating_add(10).min(95);
        }
    }

    pub async fn mark_success(&self, id: &str, task: &str, load: Option<u8>) {
        let now = chrono::Utc::now().timestamp();
        let mut agents = self.agents.lock().await;
        if let Some(a) = agents.get_mut(id) {
            a.last_success_at = Some(now);
            a.last_error = None;
            a.success_count = a.success_count.saturating_add(1);
            if let Some(l) = load {
                a.load = l.min(100);
            } else if a.load > 0 {
                a.load = a.load.saturating_sub(15);
            }
            a.current_task = task.into();
            a.status = if a.enabled {
                AgentOperationalStatus::Active
            } else {
                AgentOperationalStatus::Paused
            };
        }
    }

    pub async fn mark_error(&self, id: &str, err: &str) {
        let now = chrono::Utc::now().timestamp();
        let mut agents = self.agents.lock().await;
        if let Some(a) = agents.get_mut(id) {
            a.status = AgentOperationalStatus::Error;
            a.last_run_at = Some(now);
            a.last_error = Some(err.to_string());
            a.error_count = a.error_count.saturating_add(1);
            a.current_task = format!("Error: {}", crate::utils::clip(err, 120));
            a.load = 0;
        }
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> bool {
        let mut agents = self.agents.lock().await;
        let Some(a) = agents.get_mut(id) else {
            return false;
        };
        a.enabled = enabled;
        a.status = if enabled {
            if a.last_success_at.is_some() || a.run_count > 0 {
                AgentOperationalStatus::Active
            } else {
                AgentOperationalStatus::Standby
            }
        } else {
            AgentOperationalStatus::Paused
        };
        if !enabled {
            a.current_task = "Paused by operator".into();
            a.load = 0;
        }
        if id == THREAT_HUNTER_ID {
            self.hunter_enabled.store(enabled, Ordering::SeqCst);
        }
        true
    }

    pub async fn is_known(&self, id: &str) -> bool {
        self.agents.lock().await.contains_key(id)
    }

    /// Build dashboard JSON (compatible with existing Agents UI fields).
    pub async fn dashboard_agents(&self, ctx: AgentDashboardContext) -> Vec<serde_json::Value> {
        let black_n = ctx.black_kb;
        let fusion_n = ctx.fusion_clusters;
        let honeypots_n = ctx.honeypots;
        let healing_ready = ctx.healing_ready;
        let bdu_meta = ctx.last_scout;
        let last_react = ctx.last_react;
        let react_ready = ctx.react_ready;

        let hunter_findings = if let Some(h) = self.hunter.lock().await.as_ref() {
            h.findings_count().await
        } else {
            0
        };

        let agents = self.agents.lock().await;
        let mut out = Vec::new();

        if let Some(scout) = agents.get(SCOUT_ID) {
            let (task, load, iterations) = if scout.status == AgentOperationalStatus::Running {
                (scout.current_task.clone(), scout.load, scout.run_count as u32)
            } else if let Some(meta) = &bdu_meta {
                (
                    format!(
                        "Last SCOUT: {} BDU | heal {}/{} | fusion +{}",
                        meta.found, meta.healing_applied, meta.healing_attempted, meta.fusion_updated
                    ),
                    ((meta.found.min(100)) as u8).max(scout.load),
                    meta.found as u32,
                )
            } else {
                (
                    format!("Black KB: {} entries — ready for SCOUT", black_n),
                    (black_n.min(100)) as u8,
                    black_n as u32,
                )
            };
            let status = effective_status(scout);
            out.push(agent_json(
                SCOUT_ID,
                &scout.name,
                &scout.role,
                status,
                scout.enabled,
                &task,
                load,
                iterations,
                scout,
                Some(serde_json::json!({
                    "black_kb": black_n,
                    "last_scout_at": bdu_meta.as_ref().map(|m| m.completed_at),
                    "last_scout_status": bdu_meta.as_ref().map(|m| m.status.clone()),
                })),
            ));
        }

        if let Some(hunter) = agents.get(THREAT_HUNTER_ID) {
            let load = (hunter_findings.min(100)) as u8;
            let task = if hunter.status == AgentOperationalStatus::Running {
                hunter.current_task.clone()
            } else {
                format!(
                    "Findings buffer: {} | interval 300s | fusion {}",
                    hunter_findings,
                    if fusion_n > 0 || healing_ready {
                        "linked"
                    } else {
                        "standby"
                    }
                )
            };
            out.push(agent_json(
                THREAT_HUNTER_ID,
                &hunter.name,
                &hunter.role,
                effective_status(hunter),
                hunter.enabled,
                &task,
                load.max(hunter.load),
                hunter_findings as u32,
                hunter,
                Some(serde_json::json!({ "findings": hunter_findings })),
            ));
        }

        if let Some(healer) = agents.get(HEALER_ID) {
            let (task, load, iters) = if healer.status == AgentOperationalStatus::Running {
                (healer.current_task.clone(), healer.load, healer.run_count as u32)
            } else if let Some(meta) = &bdu_meta {
                (
                    if healing_ready {
                        format!(
                            "Last heal {}/{} applied | orchestrator ready",
                            meta.healing_applied, meta.healing_attempted
                        )
                    } else {
                        "HealingOrchestrator not initialized".into()
                    },
                    ((meta.healing_applied * 33).min(100)) as u8,
                    meta.healing_attempted as u32,
                )
            } else if healing_ready {
                (
                    "Ready — critical BDU patches on next SCOUT".into(),
                    0u8,
                    0u32,
                )
            } else {
                ("Standby — healing runtime missing".into(), 0u8, 0u32)
            };
            let status = if !healing_ready {
                "standby"
            } else {
                effective_status(healer)
            };
            out.push(agent_json(
                HEALER_ID,
                &healer.name,
                &healer.role,
                status,
                healer.enabled && healing_ready,
                &task,
                load,
                iters,
                healer,
                Some(serde_json::json!({
                    "orchestrator": healing_ready,
                    "last_heal_applied": bdu_meta.as_ref().map(|m| m.healing_applied),
                    "last_heal_attempted": bdu_meta.as_ref().map(|m| m.healing_attempted),
                })),
            ));
        }

        if let Some(mtd) = agents.get(MTD_ID) {
            let load = (honeypots_n.min(100)) as u8;
            let task = if mtd.status == AgentOperationalStatus::Running {
                mtd.current_task.clone()
            } else {
                format!(
                    "Honeypots active: {} | background mutation 300s",
                    honeypots_n
                )
            };
            out.push(agent_json(
                MTD_ID,
                &mtd.name,
                &mtd.role,
                effective_status(mtd),
                mtd.enabled,
                &task,
                load.max(mtd.load),
                honeypots_n as u32,
                mtd,
                Some(serde_json::json!({
                    "honeypots": honeypots_n,
                    "fusion_clusters": fusion_n,
                })),
            ));
        }

        drop(agents);

        if let Some(r) = &last_react {
            out.push(serde_json::json!({
                "id": REACT_ID,
                "name": "REACT++",
                "role": "AUTONOMOUS",
                "status": if react_ready { "active" } else { "paused" },
                "enabled": react_ready,
                "load": (r.iterations_used as usize * 15).min(100),
                "critic": if r.success { 0.25 } else { 0.72 },
                "current_task": r.mission,
                "react_iterations": r.iterations_used,
                "last_critic_score": if r.success { "SUCCESS" } else { "FAILED" },
                "critic_decisions": r.iterations_used,
                "mcts_nodes": 25,
                "best_path_score": format!("{:.0}%", if r.success { 100.0 } else { 0.0 }),
                "last_completed_at": r.completed_at,
                "run_count": 1,
                "success_count": if r.success { 1 } else { 0 },
                "error_count": if r.success { 0 } else { 1 },
            }));
        } else {
            out.push(serde_json::json!({
                "id": REACT_ID,
                "name": "REACT++",
                "role": "AUTONOMOUS",
                "status": if react_ready { "active" } else { "paused" },
                "enabled": react_ready,
                "load": 0,
                "critic": 0.0,
                "current_task": if react_ready {
                    "Ready — launch REACT MISSION"
                } else {
                    "Runtime not initialized"
                },
                "react_iterations": 0,
                "last_critic_score": "—",
                "critic_decisions": 0,
                "mcts_nodes": 25,
                "best_path_score": "—",
            }));
        }

        out
    }
}

fn effective_status(a: &AgentRuntime) -> &'static str {
    if !a.enabled {
        return AgentOperationalStatus::Paused.as_str();
    }
    a.status.as_str()
}

fn agent_json(
    id: &str,
    name: &str,
    role: &str,
    status: &str,
    enabled: bool,
    current_task: &str,
    load: u8,
    react_iterations: u32,
    rt: &AgentRuntime,
    metrics: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut v = serde_json::json!({
        "id": id,
        "name": name,
        "role": role,
        "status": status,
        "enabled": enabled,
        "current_task": current_task,
        "load": load,
        "critic": 0.0,
        "react_iterations": react_iterations,
        "last_critic_score": if rt.last_error.is_some() { "ERROR" } else { "ops" },
        "critic_decisions": rt.success_count,
        "mcts_nodes": 0,
        "best_path_score": "—",
        "run_count": rt.run_count,
        "success_count": rt.success_count,
        "error_count": rt.error_count,
        "last_run_at": rt.last_run_at,
        "last_success_at": rt.last_success_at,
        "last_error": rt.last_error,
    });
    if let Some(m) = metrics {
        v["metrics"] = m;
    }
    v
}
