//! Autonomous Remediation Pipeline (Phase 3)
//!
//! Позволяет системе самостоятельно применять исправления Low и Medium риска
//! без участия человека, сохраняя HITL только для High/Critical.
//!
//! Это ключевой шаг к "иммунной системе" — минимальное вмешательство человека.

use std::sync::Arc;

use crate::audit::AuditTrail;
use crate::healing_orchestrator::{HealingOrchestrator, HealingResult, PatchRisk, PatchType};

/// Политика автономного лечения.
#[derive(Debug, Clone)]
pub struct AutonomousPolicy {
    pub auto_apply_low: bool,
    pub auto_apply_medium: bool,
    pub max_risk_for_auto: f64, // 0.0 - 1.0
}

impl Default for AutonomousPolicy {
    fn default() -> Self {
        Self {
            auto_apply_low: true,
            auto_apply_medium: false, // По умолчанию Medium всё ещё требует верификации
            max_risk_for_auto: 0.4,
        }
    }
}

/// Autonomous Remediation Pipeline.
pub struct AutonomousRemediation {
    healing: Arc<HealingOrchestrator>,
    policy: AutonomousPolicy,
    audit: Arc<AuditTrail>,
}

impl AutonomousRemediation {
    pub fn new(healing: Arc<HealingOrchestrator>, audit: Arc<AuditTrail>) -> Self {
        Self {
            healing,
            policy: AutonomousPolicy::default(),
            audit,
        }
    }

    /// Пытается автоматически вылечить аномалию.
    /// Возвращает результат, если лечение было применено автономно.
    pub async fn try_auto_heal(&self, anomaly: &str, patch_type: PatchType) -> Option<HealingResult> {
        // 1. Прогоняем через Healing Orchestrator (он уже делает verification + sandbox)
        let result = match self.healing.heal(anomaly, patch_type).await {
            Ok(r) => r,
            Err(_) => return None,
        };

        // 2. Решаем, можно ли применить автоматически
        let can_auto_apply = match result.risk {
            PatchRisk::Low if self.policy.auto_apply_low => true,
            PatchRisk::Medium if self.policy.auto_apply_medium => true,
            _ => false,
        };

        if can_auto_apply && result.verification_passed && result.sandbox_passed {
            let _ = self.audit.log_event(
                "autonomous_remediation",
                &format!("auto_applied patch_id={} risk={:?}", result.patch_id, result.risk),
                0.25,
                true,
            );
            Some(result)
        } else {
            // Для High/Critical — просто логируем, что требуется HITL
            if matches!(result.risk, PatchRisk::High | PatchRisk::Critical) {
                let _ = self.audit.log_event(
                    "autonomous_remediation",
                    &format!("hitl_required patch_id={} risk={:?}", result.patch_id, result.risk),
                    0.6,
                    false,
                );
            }
            None
        }
    }
}
