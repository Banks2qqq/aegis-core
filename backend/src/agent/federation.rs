//! Federation Layer — синхронизация знаний между нодами AEGIS.

use std::sync::Arc;
use sha2::Digest;

use crate::audit::AuditTrail;
use crate::dna_engine::DnaEngine;
use crate::knowledge::KnowledgeBase;

pub struct FederationLayer {
    pub kb: Arc<KnowledgeBase>,
    #[allow(dead_code)]
    dna: Arc<DnaEngine>,
    audit: Arc<AuditTrail>,
}

impl FederationLayer {
    pub fn new(kb: Arc<KnowledgeBase>, dna: Arc<DnaEngine>, audit: Arc<AuditTrail>) -> Self {
        Self { kb, dna, audit }
    }

    /// Возвращает Merkle Root текущего состояния знаний.
    /// 
    /// Использует content_hash из БД (если есть) или хэширует содержимое.
    /// Работает эффективно даже при сотнях тысяч записей.
    pub async fn get_merkle_root(&self) -> Result<String, String> {
        // Получаем только лёгкие данные (id + content_hash)
        let hashes = self.kb.get_all_hashes().await
            .map_err(|e| format!("Failed to get hashes: {}", e))?;

        if hashes.is_empty() {
            return Ok("merkle_empty".to_string());
        }

        let mut leaf_hashes: Vec<String> = Vec::new();

        for (id, content_hash) in hashes {
            // Используем content_hash, если он есть, иначе хэшируем id
            let leaf = if !content_hash.is_empty() {
                content_hash
            } else {
                format!("{:x}", sha2::Sha256::digest(id.as_bytes()))
            };
            leaf_hashes.push(leaf);
        }

        // Сортируем для детерминированности
        leaf_hashes.sort();

        // Строим Merkle Root (простая версия — хэш от конкатенации всех листьев)
        let combined = leaf_hashes.join("");
        let root = format!("merkle_{:x}", sha2::Sha256::digest(combined.as_bytes()));

        Ok(root)
    }

    /// Синхронизация с другой нодой.
    pub async fn sync_with_peer(&self, peer_url: &str) -> Result<usize, String> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|e| format!("Failed to build client: {}", e))?;

        // 1. Получаем свой последний last_seen
        let my_last_seen = self.kb.get_last_seen().await.unwrap_or(0);

        // 2. Запрашиваем у пира записи, изменённые после my_last_seen
        let changed_resp = client
            .post(format!("{}/federation/changed_since", peer_url.trim_end_matches('/')))
            .json(&my_last_seen)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = changed_resp.status();
        let raw_text = changed_resp.text().await.map_err(|e| format!("Failed to read text: {}", e))?;

        if !status.is_success() {
            return Err(format!("Peer returned error status: {} (raw: {})", status, raw_text));
        }

        let changed: Vec<crate::knowledge_item::KnowledgeItem> = serde_json::from_str(&raw_text)
            .map_err(|e| format!("Parse failed: {} (raw: {})", e, raw_text))?;

        let mut synced = 0;

        for item in changed {
            match item.item_type {
                crate::knowledge_item::KnowledgeType::White => {
                    let _ = self.kb.ingest_white(item).await;
                }
                crate::knowledge_item::KnowledgeType::Black => {
                    let _ = self.kb.ingest_black(item).await;
                }
                _ => {}
            }
            synced += 1;
        }

        let _ = self.audit.log_event(
            "federation",
            &format!("delta_sync_completed peer={} synced={}", peer_url, synced),
            0.3,
            true,
        );

        Ok(synced)
    }

    /// Приём знаний от другой ноды.
    pub async fn ingest_federated_item(&self, item: crate::knowledge_item::KnowledgeItem) -> Result<(), String> {
        // Conflict Resolution: если запись уже существует — сравниваем last_seen
        let existing = match item.item_type {
            crate::knowledge_item::KnowledgeType::White => self.kb.get_white_by_id(&item.id).await,
            crate::knowledge_item::KnowledgeType::Black => self.kb.get_black_by_id(&item.id).await,
            _ => None,
        };

        if let Some(existing) = existing {
            if item.last_seen <= existing.last_seen {
                // Наша запись свежее — игнорируем входящую
                return Ok(());
            }
        }

        // Инжестим (побеждает более свежая запись)
        match item.item_type {
            crate::knowledge_item::KnowledgeType::White => {
                let _ = self.kb.ingest_white(item).await?;
            }
            crate::knowledge_item::KnowledgeType::Black => {
                let _ = self.kb.ingest_black(item).await?;
            }
            _ => {}
        }

        Ok(())
    }
}
