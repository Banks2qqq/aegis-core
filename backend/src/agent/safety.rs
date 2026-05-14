//! Human-in-the-Loop safety policy for AEGIS.
//!
//! Goal: even in GOD MODE the system must not execute/deploy anything
//! without explicit human approval.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    Strict,
    AuditOnly,
    Disabled,
}

/// Returns `true` when a human approval is required for this action.
///
/// Policy (paranoid defaults):
/// - Always require approval when `risk >= 0.8`
/// - Require approval for any action that looks like deploy/execute/run/shell/network mutations
pub fn require_human_approval(action: &str, risk: f64) -> bool {
    if risk >= 0.8 {
        return true;
    }

    let a = action.to_lowercase();
    let keywords = [
        "deploy",
        "execute",
        "exec",
        "run",
        "shell",
        "bash",
        "powershell",
        "curl",
        "wget",
        "apt",
        "yum",
        "dnf",
        "pip ",
        "npm ",
        "cargo ",
        "kubectl",
        "terraform",
        "ansible",
        "systemctl",
        "service ",
        "iptables",
        "netsh",
        "rm ",
        "delete",
        "chmod",
        "chown",
        "mkfs",
        "dd ",
    ];

    keywords.iter().any(|k| a.contains(k))
}

pub fn audit_hitl(audit: Option<&crate::audit::AuditTrail>, actor: &str, action: &str, risk: f64, approved: bool) {
    if let Some(audit) = audit {
        let _ = audit.log_event(actor, &format!("hitl: {}", action), risk, approved);
    }
}

