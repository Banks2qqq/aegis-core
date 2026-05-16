//! Scout 2.0 — unified open-source threat intel aggregation.

pub mod dedupe;
pub mod enrichment;
pub mod hub;
pub mod mitre;
pub mod sources;
pub mod structured;
pub mod types;
pub mod ux;

pub use enrichment::{run_enrichment_pipeline, EnrichmentReport};
pub use hub::{run_intel_collection, ScoutCollectionReport};
pub use types::ScoutFinding;
