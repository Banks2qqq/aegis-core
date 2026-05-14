//! Prometheus metrics for Scout → Critic → Inquisitor → Ingest → DNA self-learning cycle.

use prometheus::{Gauge, GaugeVec, IntCounter, IntCounterVec, Opts};
use std::sync::LazyLock;

use crate::knowledge_item::{KnowledgeItem, KnowledgeType};

// --- Scout ---
static SCOUT_ITEMS_COLLECTED: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_scout_items_collected_total",
            "Knowledge items produced by Scout pipeline by item_type",
        ),
        &["type"],
    )
    .expect("metric aegis_scout_items_collected_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static SCOUT_HYPOTHESES_GENERATED: LazyLock<IntCounter> = LazyLock::new(|| {
    let c = IntCounter::with_opts(Opts::new(
        "aegis_scout_hypotheses_generated_total",
        "Hypothesis items in Scout output (counted once per pipeline run)",
    ))
    .expect("metric aegis_scout_hypotheses_generated_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static SCOUT_CLASSIFICATION: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_scout_classification_total",
            "Scout items by final classification (white|black)",
        ),
        &["type"],
    )
    .expect("metric aegis_scout_classification_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static SCOUT_AVG_CONFIDENCE: LazyLock<Gauge> = LazyLock::new(|| {
    let g = Gauge::with_opts(Opts::new(
        "aegis_scout_avg_confidence",
        "Average confidence of items from last Scout run",
    ))
    .expect("metric aegis_scout_avg_confidence");
    let _ = prometheus::default_registry().register(Box::new(g.clone()));
    g
});

// --- Critic / Inquisitor ---
static CRITIC_VERDICT: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_critic_verdict_total",
            "Critic merged bulk gate verdict counts",
        ),
        &["verdict"],
    )
    .expect("metric aegis_critic_verdict_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static CRITIC_KNOWLEDGE_VERDICT: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_critic_knowledge_verdict_total",
            "Per-item Critic 2.0 verdict counts",
        ),
        &["verdict"],
    )
    .expect("metric aegis_critic_knowledge_verdict_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static INQUISITOR_VERDICT: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_inquisitor_verdict_total",
            "Inquisitor merged bulk gate verdict counts",
        ),
        &["verdict"],
    )
    .expect("metric aegis_inquisitor_verdict_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static INQUISITOR_KNOWLEDGE_VERDICT: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_inquisitor_knowledge_verdict_total",
            "Per-item Inquisitor 2.0 verdict counts",
        ),
        &["verdict"],
    )
    .expect("metric aegis_inquisitor_knowledge_verdict_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

// --- HITL ---
static HITL_APPROVALS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_hitl_approvals_total",
            "Human-in-the-loop approvals by stage (scout_start|ingest|other)",
        ),
        &["stage"],
    )
    .expect("metric aegis_hitl_approvals_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static HITL_REJECTIONS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_hitl_rejections_total",
            "Human-in-the-loop rejections by stage",
        ),
        &["stage"],
    )
    .expect("metric aegis_hitl_rejections_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

// --- Ingest / DNA ---
static KNOWLEDGE_INGESTED: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_knowledge_ingested_total",
            "Successful new inserts into knowledge_items (not dedup merge)",
        ),
        &["type"],
    )
    .expect("metric aegis_knowledge_ingested_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static KNOWLEDGE_DEDUPED: LazyLock<IntCounter> = LazyLock::new(|| {
    let c = IntCounter::with_opts(Opts::new(
        "aegis_knowledge_deduped_total",
        "Ingests merged into existing row (content_hash dedup)",
    ))
    .expect("metric aegis_knowledge_deduped_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static DNA_ITEMS_GAUGE: LazyLock<GaugeVec> = LazyLock::new(|| {
    let g = GaugeVec::new(
        Opts::new(
            "aegis_dna_items_total",
            "DNA snapshot item counts by type (gauge, last update)",
        ),
        &["type"],
    )
    .expect("metric aegis_dna_items_total");
    let _ = prometheus::default_registry().register(Box::new(g.clone()));
    g
});

static DNA_AVG_CONFIDENCE: LazyLock<Gauge> = LazyLock::new(|| {
    let g = Gauge::with_opts(Opts::new(
        "aegis_dna_avg_confidence",
        "DNA snapshot avg_confidence (latest)",
    ))
    .expect("metric aegis_dna_avg_confidence");
    let _ = prometheus::default_registry().register(Box::new(g.clone()));
    g
});

static DNA_LAST_UPDATE_ITEMS_ADDED: LazyLock<IntCounter> = LazyLock::new(|| {
    let c = IntCounter::with_opts(Opts::new(
        "aegis_dna_last_update_items_added",
        "Cumulative non-negative delta of total_items per DNA update",
    ))
    .expect("metric aegis_dna_last_update_items_added");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static SELF_LEARNING_CYCLE_COMPLETED: LazyLock<IntCounter> = LazyLock::new(|| {
    let c = IntCounter::with_opts(Opts::new(
        "aegis_self_learning_cycle_completed_total",
        "Scout→DNA cycles completed successfully",
    ))
    .expect("metric aegis_self_learning_cycle_completed_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static SELF_LEARNING_GATE_ATTEMPT: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_self_learning_gate_attempts_total",
            "Gate attempts for pass-rate (with aegis_self_learning_gate_passes_total)",
        ),
        &["stage"],
    )
    .expect("metric aegis_self_learning_gate_attempts_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

static SELF_LEARNING_GATE_PASS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_self_learning_gate_passes_total",
            "Gate passes (same stage labels as attempts)",
        ),
        &["stage"],
    )
    .expect("metric aegis_self_learning_gate_passes_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

/// EMA (0–1) of gate success for quick dashboards; exact rates from passes/attempts counters.
static SELF_LEARNING_PASS_RATE: LazyLock<GaugeVec> = LazyLock::new(|| {
    let g = GaugeVec::new(
        Opts::new(
            "aegis_self_learning_pass_rate",
            "Exponential moving average of gate pass by stage",
        ),
        &["stage"],
    )
    .expect("metric aegis_self_learning_pass_rate");
    let _ = prometheus::default_registry().register(Box::new(g.clone()));
    g
});

static FEEDBACK_RECEIVED: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let c = IntCounterVec::new(
        Opts::new(
            "aegis_feedback_received_total",
            "Human feedback recorded on knowledge_items",
        ),
        &["type"],
    )
    .expect("metric aegis_feedback_received_total");
    let _ = prometheus::default_registry().register(Box::new(c.clone()));
    c
});

fn norm_verdict(v: &str) -> &'static str {
    match v.to_ascii_lowercase().as_str() {
        "allow" => "allow",
        "escalate" => "escalate",
        "block" => "block",
        _ => "escalate",
    }
}

fn hitl_stage_label(s: &str) -> &'static str {
    match s {
        "scout_start" => "scout_start",
        "ingest" => "ingest",
        _ => "other",
    }
}

fn type_label(t: &KnowledgeType) -> &'static str {
    match t {
        KnowledgeType::White => "white",
        KnowledgeType::Black => "black",
        KnowledgeType::Hypothesis => "hypothesis",
        KnowledgeType::TTP => "ttp",
    }
}

/// Scout pipeline completed (items returned).
pub fn record_scout_run(items: &[KnowledgeItem]) {
    for it in items {
        SCOUT_ITEMS_COLLECTED
            .with_label_values(&[type_label(&it.item_type)])
            .inc();
        match it.item_type {
            KnowledgeType::White => SCOUT_CLASSIFICATION.with_label_values(&["white"]).inc(),
            KnowledgeType::Black => SCOUT_CLASSIFICATION.with_label_values(&["black"]).inc(),
            _ => {}
        }
    }
    let hyp_n = items
        .iter()
        .filter(|i| i.item_type == KnowledgeType::Hypothesis)
        .count() as u64;
    SCOUT_HYPOTHESES_GENERATED.inc_by(hyp_n);
    let avg = if items.is_empty() {
        0.0
    } else {
        items.iter().map(|i| i.confidence).sum::<f64>() / items.len() as f64
    };
    SCOUT_AVG_CONFIDENCE.set(avg);
}

pub fn critic_bulk_verdict(verdict: &str) {
    CRITIC_VERDICT
        .with_label_values(&[norm_verdict(verdict)])
        .inc();
}

pub fn critic_knowledge_verdict(verdict: &str) {
    CRITIC_KNOWLEDGE_VERDICT
        .with_label_values(&[norm_verdict(verdict)])
        .inc();
}

pub fn inquisitor_bulk_verdict(verdict: &str) {
    INQUISITOR_VERDICT
        .with_label_values(&[norm_verdict(verdict)])
        .inc();
}

pub fn inquisitor_knowledge_verdict(verdict: &str) {
    INQUISITOR_KNOWLEDGE_VERDICT
        .with_label_values(&[norm_verdict(verdict)])
        .inc();
}

pub fn hitl_approval(stage: &str) {
    HITL_APPROVALS
        .with_label_values(&[hitl_stage_label(stage)])
        .inc();
}

pub fn hitl_rejection(stage: &str) {
    HITL_REJECTIONS
        .with_label_values(&[hitl_stage_label(stage)])
        .inc();
}

pub fn knowledge_ingested(ty: &KnowledgeType, n: u64) {
    if n == 0 {
        return;
    }
    KNOWLEDGE_INGESTED
        .with_label_values(&[type_label(ty)])
        .inc_by(n);
}

pub fn knowledge_deduped(n: u64) {
    KNOWLEDGE_DEDUPED.inc_by(n);
}

pub fn dna_snapshot_update(
    white: usize,
    black: usize,
    hypothesis: usize,
    ttp: usize,
    avg_confidence: f64,
    last_delta_items: u64,
) {
    DNA_ITEMS_GAUGE
        .with_label_values(&["white"])
        .set(white as f64);
    DNA_ITEMS_GAUGE
        .with_label_values(&["black"])
        .set(black as f64);
    DNA_ITEMS_GAUGE
        .with_label_values(&["hypothesis"])
        .set(hypothesis as f64);
    DNA_ITEMS_GAUGE
        .with_label_values(&["ttp"])
        .set(ttp as f64);
    DNA_AVG_CONFIDENCE.set(avg_confidence);
    DNA_LAST_UPDATE_ITEMS_ADDED.inc_by(last_delta_items);
}

pub fn self_learning_cycle_completed() {
    SELF_LEARNING_CYCLE_COMPLETED.inc();
}

/// One gate evaluation: increments attempts, optionally passes, updates EMA `aegis_self_learning_pass_rate`.
pub fn learning_gate_finish(stage: &str, pass: bool) {
    SELF_LEARNING_GATE_ATTEMPT.with_label_values(&[stage]).inc();
    if pass {
        SELF_LEARNING_GATE_PASS.with_label_values(&[stage]).inc();
    }
    let g = SELF_LEARNING_PASS_RATE.with_label_values(&[stage]);
    let prev = g.get();
    let alpha = 0.25;
    let x = if pass { 1.0 } else { 0.0 };
    let next = if prev == 0.0 {
        x
    } else {
        prev * (1.0 - alpha) + x * alpha
    };
    g.set(next.clamp(0.0, 1.0));
}

pub fn feedback_received(label: &str) {
    FEEDBACK_RECEIVED.with_label_values(&[label]).inc();
}

/// Severity (0.0–1.0) from Formal Verification Critic during healing.
pub fn healing_verification_severity(severity: f64) {
    static HEALING_VERIFICATION_SEVERITY: LazyLock<Gauge> = LazyLock::new(|| {
        Gauge::with_opts(Opts::new(
            "aegis_healing_verification_severity",
            "Severity score from Formal Verification Critic (0.0 = clean, 1.0 = critical)",
        ))
        .expect("failed to create healing_verification_severity gauge")
    });
    HEALING_VERIFICATION_SEVERITY.set(severity);
}

/// Increments when a healing patch is generated (Inquisitor + DNA).
pub fn healing_patch_generated() {
    static HEALING_PATCH_GENERATED: LazyLock<IntCounter> = LazyLock::new(|| {
        IntCounter::new(
            "aegis_healing_patch_generated_total",
            "Total number of healing patches generated by Patch Generator (Inquisitor + DNA)",
        )
        .expect("failed to create healing_patch_generated counter")
    });
    HEALING_PATCH_GENERATED.inc();
}

/// Records sandbox test duration and level for healing patches.
pub fn healing_sandbox_test(duration_secs: f64, level: &str) {
    static HEALING_SANDBOX_DURATION: LazyLock<GaugeVec> = LazyLock::new(|| {
        GaugeVec::new(
            Opts::new(
                "aegis_healing_sandbox_duration_seconds",
                "Duration of sandbox test for healing patches",
            ),
            &["level"],
        )
        .expect("failed to create healing_sandbox_duration gaugevec")
    });
    HEALING_SANDBOX_DURATION.with_label_values(&[level]).set(duration_secs);
}

/// Increments when a TTP is extracted from a honeypot interaction.
pub fn honeypot_ttp_extracted() {
    static HONEYPOT_TTP_EXTRACTED: LazyLock<IntCounter> = LazyLock::new(|| {
        IntCounter::new(
            "aegis_honeypot_ttp_extracted_total",
            "Total TTPs extracted from high-interaction honeypot sessions",
        )
        .expect("failed to create honeypot_ttp_extracted counter")
    });
    HONEYPOT_TTP_EXTRACTED.inc();
}

pub fn verification_severity(severity: f64) {
    healing_verification_severity(severity);
}

pub fn healing_completed(applied: bool, risk_label: &str) {
    static HEALING_COMPLETED: LazyLock<IntCounterVec> = LazyLock::new(|| {
        let c = IntCounterVec::new(
            Opts::new(
                "aegis_healing_completed_total",
                "Total number of healing cycles completed",
            ),
            &["applied", "risk"],
        )
        .expect("failed to create healing_completed counter");
        let _ = prometheus::default_registry().register(Box::new(c.clone()));
        c
    });
    HEALING_COMPLETED.with_label_values(&[if applied { "true" } else { "false" }, risk_label]).inc();
}

/// Raft 2.0 / Distributed Oracle: события по фазам (`append`, `commit`, `apply`).
pub fn raft_phase(phase: &'static str) {
    static RAFT_PHASE: LazyLock<IntCounterVec> = LazyLock::new(|| {
        let c = IntCounterVec::new(
            Opts::new(
                "aegis_raft_phase_total",
                "Distributed Oracle Raft-like log replication and state machine phases",
            ),
            &["phase"],
        )
        .expect("metric aegis_raft_phase_total");
        let _ = prometheus::default_registry().register(Box::new(c.clone()));
        c
    });
    RAFT_PHASE.with_label_values(&[phase]).inc();
}
