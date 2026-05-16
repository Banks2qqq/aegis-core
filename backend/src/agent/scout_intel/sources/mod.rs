//! Per-source collectors (open APIs + keyed aggregators).

pub mod abuse_ch;
pub mod feed_parse;
pub mod fortiguard;
pub mod fstec;
pub mod http_util;
pub mod otx;
pub mod safe_surf;
pub mod talos;
pub mod virustotal;

use async_trait::async_trait;

use super::types::ScoutFinding;

#[derive(Debug, Clone)]
pub struct SourceMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub region: &'static str,
    pub needs_api_key: bool,
}

#[async_trait]
pub trait ScoutSource: Send + Sync {
    fn meta(&self) -> SourceMeta;
    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String>;
}

pub fn all_sources() -> Vec<Box<dyn ScoutSource>> {
    vec![
        Box::new(fstec::FstecBduSource),
        Box::new(abuse_ch::ThreatFoxSource),
        Box::new(abuse_ch::UrlhausSource),
        Box::new(abuse_ch::MalwareBazaarSource),
        Box::new(otx::OtxSource),
        Box::new(virustotal::VirusTotalSource),
        Box::new(talos::TalosBlocklistSource),
        Box::new(fortiguard::FortiGuardOutbreakSource),
        Box::new(safe_surf::SafeSurfNkckiSource),
    ]
}
