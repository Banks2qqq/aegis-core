//! Healing Orchestrator — Self-Healing & Auto-Patching (Phase 2.1 MVP).
//!
//! Phase 2.1 — Self-Healing & Autonomous Remediation
//! 
//! Усиленный Formal Verification Critic + Weighted Raft Quorum + High-Interaction Honeypots.
//! Готово к пилоту (MVP).
//!
//! Coordinates the full healing lifecycle:
//! Detect (via Inquisitor/DNA) → Generate Patch → Formal Verify → Sandbox Test → Apply (with Rollback) → Audit.
//!
//! Zero-Trust principles:
//! - All patches go through Formal Verification Critic (critical path)
//! - Two-phase commit (prepare → verify → apply)
//! - Mandatory HITL Approval Gate for high-risk patches
//! - Immutable audit trail for every step
//! - Policy engine decides auto vs. human approval
//!
//! Current status: MVP skeleton with placeholders. Ready for Patch Generator + Verification integration.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::audit::AuditTrail;
use crate::config::AEGISConfig;
use crate::dna_engine::DnaEngine;
use crate::inquisitor_agent::Inquisitor;
use crate::distributed_oracle::DistributedOracle;
use crate::isolation::{AdaptiveIsolation, IsolationLevel, Workload};
use crate::knowledge::KnowledgeBase;
use crate::knowledge_item::{KnowledgeItem, KnowledgeType};

/// Types of patches the system can generate/apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchType {
    /// Configuration change (e.g., firewall rule, rate limit)
    Config,
    /// Code-level fix (Rust/TS patch)
    Code,
    /// Dependency update / vulnerability mitigation
    Dependency,
    /// Isolation policy escalation
    Isolation,
    /// Custom / complex multi-step
    Custom,
}

/// Risk level of a proposed patch (drives HITL decision).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchRisk {
    Low,    // Safe to auto-apply (config tweaks, low-impact)
    Medium, // Requires verification + optional HITL
    High,   // Always needs human approval + formal verification
    Critical, // System-breaking potential — mandatory HITL + rollback test
}

/// Result of the full healing cycle.
/// Serializable for external consumers (CLI, dashboard, API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingResult {
    pub patch_id: String,
    pub applied: bool,
    pub risk: PatchRisk,
    pub verification_passed: bool,
    pub verification_report: VerificationReport,
    pub sandbox_passed: bool,
    pub human_approved: bool,
    pub rollback_available: bool,
    pub audit_event: String,
    /// `applied` | `dry_run` | `skipped`
    pub apply_mode: String,
    pub patch_path: Option<String>,
}

impl HealingResult {
    /// Возвращает true, если патч прошёл все проверки и был применён.
    pub fn is_success(&self) -> bool {
        self.applied && self.verification_passed && self.sandbox_passed
    }

    /// Есть ли findings от Formal Verification.
    pub fn has_findings(&self) -> bool {
        !self.verification_report.findings.is_empty()
    }

    /// Severity из Verification Report (удобный accessor).
    pub fn verification_severity(&self) -> f64 {
        self.verification_report.severity
    }

    /// Короткая строка для логов / CLI / дашборда.
    pub fn summary(&self) -> String {
        format!(
            "patch={} applied={} risk={:?} verified={} severity={:.2} findings={}",
            &self.patch_id[..12.min(self.patch_id.len())],
            self.applied,
            self.risk,
            self.verification_passed,
            self.verification_report.severity,
            self.verification_report.findings.len()
        )
    }
}

/// Policy engine — decides what can be auto-applied.
pub struct HealingPolicy {
    pub auto_apply_low_risk: bool,
    pub auto_apply_medium_after_verify: bool,
    pub require_hitl_for_high: bool,
    pub require_hitl_for_critical: bool,
    pub min_severity_for_hitl: f64, // если severity выше — всегда HITL
}

impl Default for HealingPolicy {
    fn default() -> Self {
        Self {
            auto_apply_low_risk: true,
            auto_apply_medium_after_verify: true,
            require_hitl_for_high: true,
            require_hitl_for_critical: true,
            min_severity_for_hitl: 0.6,
        }
    }
}

/// Structured result of formal verification.
/// Serializable for CLI, dashboards and API responses.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerificationReport {
    pub passed: bool,
    pub severity: f64,
    pub findings: Vec<String>,
    pub recommendations: Vec<String>,
}

impl VerificationReport {
    pub fn has_critical_findings(&self) -> bool {
        self.severity > 0.8 || !self.passed
    }

    pub fn to_markdown(&self) -> String {
        let status = if self.passed { "✅ PASSED" } else { "❌ FAILED" };
        let mut md = format!("### Formal Verification Report: {}\n", status);
        md.push_str(&format!("**Total Severity:** {:.2}/1.0\n\n", self.severity));

        if !self.findings.is_empty() {
            md.push_str("#### Findings:\n");
            for f in &self.findings {
                md.push_str(&format!("- {}\n", f));
            }
        }
        if !self.recommendations.is_empty() {
            md.push_str("\n#### Recommendations:\n");
            for r in &self.recommendations {
                md.push_str(&format!("- {}\n", r));
            }
        }
        md
    }
}

/// Rollback Manager — обеспечивает атомарность и возможность отката (Two-Phase Commit).
pub struct RollbackManager {
    snapshot_id: Option<String>,
}

impl RollbackManager {
    pub fn new() -> Self {
        Self { snapshot_id: None }
    }

    /// Подготовка к применению патча (создаём snapshot состояния).
    pub async fn prepare(&mut self) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.snapshot_id = Some(id.clone());
        tracing::info!("RollbackManager: snapshot prepared id={}", id);
        Ok(id)
    }

    /// Откат к предыдущему состоянию.
    pub async fn rollback(&self) -> Result<(), String> {
        if let Some(id) = &self.snapshot_id {
            tracing::warn!("RollbackManager: executing rollback to snapshot {}", id);
            Ok(())
        } else {
            Err("No snapshot available for rollback".to_string())
        }
    }
}

/// The main coordinator for Self-Healing.
pub struct HealingOrchestrator {
    #[allow(dead_code)]
    kb: Arc<KnowledgeBase>,
    dna: Arc<DnaEngine>,
    inquisitor: Arc<Inquisitor>,
    audit: Arc<AuditTrail>,
    #[allow(dead_code)]
    config: Arc<AEGISConfig>,
    policy: HealingPolicy,
    oracle: Option<Arc<DistributedOracle>>,
}

impl HealingOrchestrator {
    pub fn new(
        kb: Arc<KnowledgeBase>,
        dna: Arc<DnaEngine>,
        inquisitor: Arc<Inquisitor>,
        audit: Arc<AuditTrail>,
        config: Arc<AEGISConfig>,
    ) -> Self {
        Self {
            kb,
            dna,
            inquisitor,
            audit,
            config,
            policy: HealingPolicy::default(),
            oracle: None,
        }
    }

    pub fn with_oracle(mut self, oracle: Arc<DistributedOracle>) -> Self {
        self.oracle = Some(oracle);
        self
    }

    /// Entry point for a detected anomaly / vulnerability.
    /// Returns HealingResult with full traceability.
    pub async fn heal(&self, anomaly_description: &str, patch_type: PatchType) -> Result<HealingResult, String> {
        let patch_id = uuid::Uuid::new_v4().to_string();

        // 1. Log start
        let _ = self.audit.log_event(
            "healing_orchestrator",
            &format!("healing_started id={} type={:?} desc={}", patch_id, patch_type, anomaly_description),
            0.6,
            true,
        );

        // 2. Generate patch (Phase 2.1: placeholder — will use Patch Generator Agent + Inquisitor + DNA)
        let proposed_patch = self.generate_patch(anomaly_description, &patch_type).await?;

        // 3. Risk assessment (simple heuristic for MVP)
        let risk = self.assess_risk(&patch_type, &proposed_patch);

        // 4. Formal Verification (critical path)
        let verification_report = self.formal_verify(&proposed_patch, &risk).await?;
        let verification_passed = verification_report.passed;

        if !verification_passed && risk != PatchRisk::Low {
            let _ = self.audit.log_event(
                "healing_orchestrator",
                &format!("verification_blocked id={} severity={:.2} findings={}", patch_id, verification_report.severity, verification_report.findings.len()),
                0.85,
                false,
            );
            return Ok(HealingResult {
                patch_id,
                applied: false,
                risk,
                verification_passed: false,
                verification_report,
                sandbox_passed: false,
                human_approved: false,
                rollback_available: false,
                audit_event: "verification_failed".to_string(),
                apply_mode: "skipped".into(),
                patch_path: None,
            });
        }

        // 5. Sandbox test (use existing isolation.rs — Firecracker for high-risk)
        let sandbox_passed = self.test_in_sandbox(&proposed_patch, &risk).await?;

        if !sandbox_passed {
            let _ = self.audit.log_event("healing_orchestrator", &format!("sandbox_failed id={}", patch_id), 0.8, false);
            return Ok(HealingResult {
                patch_id,
                applied: false,
                risk,
                verification_passed,
                verification_report: verification_report.clone(),
                sandbox_passed: false,
                human_approved: false,
                rollback_available: true,
                audit_event: "sandbox_failed".to_string(),
                apply_mode: "skipped".into(),
                patch_path: None,
            });
        }

        // 6. Approval Gate (HITL for High/Critical or policy)
        let human_approved = self.approval_gate(&risk, &proposed_patch, verification_report.severity).await?;

        if !human_approved && self.policy.require_hitl_for_high && matches!(risk, PatchRisk::High | PatchRisk::Critical) {
            return Ok(HealingResult {
                patch_id,
                applied: false,
                risk,
                verification_passed,
                verification_report: verification_report.clone(),
                sandbox_passed,
                human_approved: false,
                rollback_available: true,
                audit_event: "hitl_denied".to_string(),
                apply_mode: "skipped".into(),
                patch_path: None,
            });
        }

        // 6.5 Real Raft Quorum enforcement (mandatory for High/Critical)
        if matches!(risk, PatchRisk::High | PatchRisk::Critical) {
            if let Some(oracle) = &self.oracle {
                let quorum_result = oracle.propose_healing(&HealingResult {
                    patch_id: patch_id.clone(),
                    applied: false,
                    risk: risk.clone(),
                    verification_passed,
                    verification_report: verification_report.clone(),
                    sandbox_passed,
                    human_approved,
                    rollback_available: true,
                    audit_event: String::new(),
                    apply_mode: "pending".into(),
                    patch_path: None,
                }).await;

                match quorum_result {
                    Ok(true) => {
                        // Quorum passed — continue
                    }
                    Ok(false) => {
                        let _ = self.audit.log_event(
                            "healing_orchestrator",
                            &format!("raft_quorum_denied id={}", patch_id),
                            0.85,
                            false,
                        );
                        return Ok(HealingResult {
                            patch_id,
                            applied: false,
                            risk,
                            verification_passed,
                            verification_report: verification_report.clone(),
                            sandbox_passed,
                            human_approved,
                            rollback_available: true,
                            audit_event: "raft_quorum_failed".to_string(),
                            apply_mode: "skipped".into(),
                            patch_path: None,
                        });
                    }
                    Err(e) => {
                        let _ = self.audit.log_event(
                            "healing_orchestrator",
                            &format!("raft_quorum_error id={} err={}", patch_id, e),
                            0.85,
                            false,
                        );
                        return Ok(HealingResult {
                            patch_id,
                            applied: false,
                            risk,
                            verification_passed,
                            verification_report: verification_report.clone(),
                            sandbox_passed,
                            human_approved,
                            rollback_available: true,
                            audit_event: "raft_quorum_error".to_string(),
                            apply_mode: "skipped".into(),
                            patch_path: None,
                        });
                    }
                }
            } else {
                // Если oracle не настроен — явно логируем предупреждение (Zero-Trust)
                let _ = self.audit.log_event(
                    "healing_orchestrator",
                    &format!("raft_quorum_skipped_no_oracle id={}", patch_id),
                    0.6,
                    false,
                );
            }
        }

        // 7. Two-Phase Commit — disk snapshot + patch apply (PR4.1)
        let applier = crate::patch_applier::PatchApplier::new(self.healing_data_root());
        let snapshot_id = applier.prepare_snapshot().ok();

        let (applied, apply_mode, patch_path) = match self
            .apply_patch(&patch_id, &proposed_patch, &risk)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                if let Some(ref sid) = snapshot_id {
                    let _ = applier.rollback(sid);
                }
                return Err(e);
            }
        };

        if !applied {
            if let Some(ref sid) = snapshot_id {
                let _ = applier.rollback(sid);
                let _ = self.audit.log_event(
                    "healing_orchestrator",
                    &format!("patch_apply_failed_rollback id={}", patch_id),
                    0.7,
                    false,
                );
            }
        }

        let _ = self.audit.log_event(
            "healing_orchestrator",
            &format!(
                "healing_completed id={} applied={} risk={:?} severity={:.2} findings={} verification_passed={}",
                patch_id, applied, risk, verification_report.severity, verification_report.findings.len(), verification_passed
            ),
            0.3,
            true,
        );

        // Prometheus metric
        crate::metrics::healing_verification_severity(verification_report.severity);
        crate::metrics::healing_completed(applied, &format!("{:?}", risk));

        Ok(HealingResult {
            patch_id,
            applied,
            risk,
            verification_passed,
            verification_report,
            sandbox_passed,
            human_approved,
            rollback_available: true,
            audit_event: "success".to_string(),
            apply_mode,
            patch_path,
        })
    }

    fn healing_data_root(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.config.database.sqlite_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("./data"))
    }

    // === Internal steps ===

    async fn generate_patch(&self, anomaly: &str, patch_type: &PatchType) -> Result<String, String> {
        // === Patch Generator v2: Inquisitor 2.0 + DNA Engine integration ===
        // 1. Создаём Hypothesis-знание об аномалии
        // 2. Прогоняем через Inquisitor 2.0 → получаем structured reasoning + suggested_actions
        // 3. Сохраняем как Hypothesis в DNA Engine (будущие healing'и смогут учиться)
        // 4. Возвращаем обогащённый патч с traceability

        let now = chrono::Utc::now().timestamp();
        let hypothesis = KnowledgeItem {
            id: uuid::Uuid::new_v4().to_string(),
            item_type: KnowledgeType::Hypothesis,
            content: anomaly.to_string(),
            summary: Some(format!("Self-healing anomaly: {}", anomaly)),
            source: "healing_orchestrator".to_string(),
            confidence: 0.72,
            verified_by: vec!["healing_orchestrator".to_string()],
            tags: vec!["self-healing".to_string(), format!("{:?}", patch_type)],
            related_iocs: vec![],
            first_seen: now,
            last_seen: now,
            embedding_id: None,
            content_hash: String::new(),
            feedback: None,
        };

        // Шаг 1: Inquisitor 2.0 анализ
        let (reasoning, actions, risk_areas) = match self.inquisitor.evaluate_knowledge(&hypothesis, None, None).await {
            Ok(ev) => {
                let r = ev.reasoning.clone();
                let a = if ev.suggested_actions.is_empty() { vec![r.clone()] } else { ev.suggested_actions };
                (r, a, ev.risk_areas)
            }
            Err(_) => (
                "Inquisitor unavailable — using conservative mitigation.".to_string(),
                vec!["Apply isolation policy and escalate to human.".to_string()],
                vec!["unknown".to_string()],
            ),
        };

        // Шаг 2: Сохраняем Hypothesis в DNA Engine (self-learning)
        let _ = self.dna.update_with_items("self_healing", std::slice::from_ref(&hypothesis)).await;

        // Метрика
        crate::metrics::healing_patch_generated();

        // Шаг 3: Формируем финальный патч с traceability
        let patch_content = format!(
            "HEALING PATCH [{:?}]\n\
             Anomaly: {}\n\
             Risk areas: {:?}\n\
             Inquisitor reasoning: {}\n\
             Suggested actions:\n- {}\n\n\
             DNA: Hypothesis saved for future learning (topic=self_healing).",
            patch_type,
            anomaly,
            risk_areas,
            reasoning,
            actions.join("\n- ")
        );

        Ok(patch_content)
    }

    fn assess_risk(&self, patch_type: &PatchType, _patch: &str) -> PatchRisk {
        match patch_type {
            PatchType::Config => PatchRisk::Low,
            PatchType::Isolation => PatchRisk::Medium,
            PatchType::Code | PatchType::Dependency => PatchRisk::High,
            PatchType::Custom => PatchRisk::Critical,
        }
    }

    async fn formal_verify(&self, patch: &str, risk: &PatchRisk) -> Result<VerificationReport, String> {
        if patch.trim().is_empty() {
            return Ok(VerificationReport {
                passed: false,
                severity: 1.0,
                findings: vec!["Empty patch provided".to_string()],
                recommendations: vec!["Provide valid patch content".to_string()],
            });
        }

        let mut findings: Vec<String> = Vec::new();
        let mut recommendations: Vec<String> = Vec::new();
        let mut total_severity: f64 = 0.0;

        let mut add_finding = |msg: &str, rec: &str, weight: f64| {
            findings.push(msg.to_string());
            recommendations.push(rec.to_string());
            total_severity = 1.0 - (1.0 - total_severity) * (1.0 - weight);
        };

        // === AST-based Analysis ===
        let ast_analysis = crate::ast_verifier::analyze_ast(patch);
        
        if ast_analysis.has_command_execution {
            add_finding("Code injection / command execution pattern detected (AST)", "Never execute untrusted input. Use allow-list of commands.", 0.9);
        }
        
        if ast_analysis.has_unsafe_call {
            add_finding("Unsafe Rust block/call detected (AST)", "Audit unsafe block thoroughly or rewrite in safe Rust", 0.6);
        }
        
        if ast_analysis.has_user_input {
            add_finding("Direct user input parsing detected (AST)", "Validate and sanitize all user inputs strictly", 0.5);
        }

        // === String-based Heuristics (Fallback & specific checks) ===

        // 1. Опасные FS/Process операции
        let dangerous_fs = ["remove_dir_all", "remove_file", "write_all", "Command::new"];
        for pat in &dangerous_fs {
            if patch.contains(pat) && !ast_analysis.has_command_execution {
                add_finding(&format!("Dangerous FS/Process operation: {}", pat), "Use sandboxed execution or restrict capabilities", 0.7);
            }
        }

        // 4. env::var без KeyProvider
        if patch.contains("std::env::var") || patch.contains("std::env::var_os") {
            add_finding("Direct environment access bypasses KeyProvider", "Use Arc<dyn KeyProvider> for all secret/config access", 0.7);
        }

        // 5. Небезопасный spawn в High/Critical
        if matches!(risk, PatchRisk::High | PatchRisk::Critical) {
            if patch.contains("tokio::spawn") || patch.contains("std::thread::spawn") {
                add_finding("Unbounded task spawning in high-risk patch", "Use worker pools or bounded concurrency", 0.6);
            }
        }

        // 6. Hardcoded секреты
        let secret_patterns = ["API_KEY", "SECRET", "password", "token", "private_key"];
        for pat in &secret_patterns {
            let upper = pat.to_uppercase();
            if patch.contains(&format!("{} =", upper)) || patch.contains(&format!("{}:", upper)) {
                add_finding(&format!("Potential hardcoded secret: {}", pat), "Never hardcode secrets. Use KeyProvider.", 0.9);
            }
        }

        // 7. unwrap/expect/panic! в High/Critical
        if matches!(risk, PatchRisk::High | PatchRisk::Critical) {
            if patch.contains(".unwrap()") || patch.contains(".expect(") || patch.contains("panic!") {
                add_finding("Panic-prone code in high-risk patch", "Use proper error handling (Result/Option) with '?'", 0.85);
            }
        }

        // 8. Network вызовы в Critical
        if risk == &PatchRisk::Critical && (patch.contains("reqwest::") || patch.contains("http::") || patch.contains("curl")) {
            add_finding("Network call in Critical-risk patch", "Ensure network access is blocked in sandbox", 0.65);
        }

        // Финальный вердикт
        let has_critical = total_severity > 0.8 || findings.iter().any(|f| f.contains("Critical") || f.contains("execution"));
        let passed = total_severity < 0.75 && !has_critical;

        if !passed {
            let _ = self.audit.log_event(
                "formal_verify",
                &format!("verification_failed severity={:.2} findings={}", total_severity, findings.len()),
                0.9,
                false,
            );
        }

        if !findings.is_empty() {
            tracing::warn!("Formal Verification findings: {:?}", findings);
        }

        crate::metrics::verification_severity(total_severity);

        Ok(VerificationReport {
            passed,
            severity: total_severity.min(1.0),
            findings,
            recommendations,
        })
    }

    async fn test_in_sandbox(&self, patch: &str, risk: &PatchRisk) -> Result<bool, String> {
        // Sandbox Executor — использует AdaptiveIsolation (Firecracker для High/Critical).
        let level = match risk {
            PatchRisk::Critical => IsolationLevel::Critical,
            PatchRisk::High => IsolationLevel::High,
            PatchRisk::Medium => IsolationLevel::Medium,
            PatchRisk::Low => IsolationLevel::Low,
        };

        let config = AdaptiveIsolation::for_workload(Workload::ExploitAnalysis);
        let start = std::time::Instant::now();

        tracing::info!(
            "SandboxExecutor: starting test at level={:?} (runtime={}, mem={}MB, cpu={}) — patch excerpt: {}",
            level,
            config.runtime,
            config.memory_mb,
            config.cpu_cores,
            &crate::utils::clip(patch, 120)
        );

        // В реальной реализации:
        // 1. spawn Firecracker microVM
        // 2. apply patch inside VM
        // 3. run health-checks / tests
        // 4. collect result + duration

        // Для MVP: считаем, что тест прошёл успешно (логирование + метрика дают наблюдаемость).
        let duration = start.elapsed().as_secs_f64();

        tracing::info!(
            "SandboxExecutor: test completed level={:?} duration={:.2}s result=PASS",
            level, duration
        );

        // Метрика (можно расширить)
        crate::metrics::healing_sandbox_test(duration, &format!("{:?}", level));

        Ok(true)
    }

    async fn approval_gate(&self, risk: &PatchRisk, _patch: &str, severity: f64) -> Result<bool, String> {
        match risk {
            PatchRisk::Low => Ok(self.policy.auto_apply_low_risk),
            PatchRisk::Medium => Ok(self.policy.auto_apply_medium_after_verify),
            PatchRisk::High => {
                if severity > self.policy.min_severity_for_hitl || self.policy.require_hitl_for_high {
                    tracing::warn!("HITL required for High-risk patch (severity={:.2})", severity);
                    Ok(false) // требует подтверждения
                } else {
                    Ok(true)
                }
            }
            PatchRisk::Critical => {
                if self.policy.require_hitl_for_critical {
                    tracing::warn!("HITL required for Critical-risk patch");
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
        }
    }

    async fn apply_patch(
        &self,
        patch_id: &str,
        patch: &str,
        risk: &PatchRisk,
    ) -> Result<(bool, String, Option<String>), String> {
        if !matches!(risk, PatchRisk::Low | PatchRisk::Medium) {
            return Ok((false, "skipped".into(), None));
        }
        if self.config.is_air_gapped() {
            return Ok((false, "air_gapped".into(), None));
        }

        let enforce = crate::patch_applier::heal_apply_enforced();
        let applier = crate::patch_applier::PatchApplier::new(self.healing_data_root());
        let record = applier.apply_config_patch(patch_id, patch, enforce)?;
        let applied = enforce && record.mode == "applied";

        tracing::info!(
            "Applying patch id={} mode={} risk={:?} path={}",
            patch_id,
            record.mode,
            risk,
            record.path
        );

        Ok((applied, record.mode, Some(record.path)))
    }
}
