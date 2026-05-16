use async_trait::async_trait;

use crate::fstec_bdu;

use super::{ScoutFinding, ScoutSource, SourceMeta};

pub struct FstecBduSource;

#[async_trait]
impl ScoutSource for FstecBduSource {
    fn meta(&self) -> SourceMeta {
        SourceMeta {
            id: "fstec_bdu",
            label: "ФСТЭК БДУ (bdu.fstec.ru)",
            region: "RU",
            needs_api_key: false,
        }
    }

    async fn collect(&self, limit: usize) -> Result<Vec<ScoutFinding>, String> {
        let vulns = fstec_bdu::fetch_high_and_critical(limit).await?;
        Ok(vulns
            .into_iter()
            .map(|v| ScoutFinding {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: "fstec_bdu".into(),
                source_label: "ФСТЭК БДУ".into(),
                title: format!("{} — {}", v.bdu_id, v.title),
                severity: v.severity.clone(),
                summary: format!("BDU {} | {}", v.bdu_id, v.title),
                url: Some(v.url.clone()),
                iocs: vec![v.bdu_id.clone()],
                cves: Vec::new(),
                mitre_techniques: Vec::new(),
                tags: vec!["fstec".into(), "bdu".into(), v.severity.clone()],
                ..Default::default()
            })
            .collect())
    }
}
