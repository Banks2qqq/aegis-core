use reqwest::Client;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{timeout, Duration as TokioDuration};
use crate::knowledge_item::{KnowledgeFeedback, KnowledgeItem, KnowledgeType};
use crate::audit::AuditTrail;
use std::sync::Arc;
use sha2::{Digest, Sha256};

const EMBEDDING_URL: &str = "https://openrouter.ai/api/v1/embeddings";
const QDRANT_URL: &str = "http://127.0.0.1:6333";
const COLLECTION_OSINT: &str = "aegis_osint";
const COLLECTION_DARKNET: &str = "aegis_darknet";
const COLLECTION_WHITE: &str = "white_behavior";
const COLLECTION_BLACK: &str = "black_threats";
const CHUNK_SIZE: usize = 500;
const TOP_K: usize = 5;

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f64>,
}

#[derive(Debug, Serialize)]
struct QdrantPoint {
    id: String,
    vector: Vec<f64>,
    payload: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct QdrantUpsert {
    points: Vec<QdrantPoint>,
}

#[derive(Debug, Serialize)]
struct QdrantSearch {
    vector: Vec<f64>,
    limit: usize,
    with_payload: bool,
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResponse {
    result: Vec<QdrantSearchResult>,
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResult {
    id: String,
    payload: Option<HashMap<String, String>>,
    score: f64,
}

pub struct KnowledgeBase {
    http_client: Client,
    qdrant_client: Client,
    api_key: String,
    sqlite_path: String,
    air_gapped: bool,
    qdrant_url: String,
    audit: Option<Arc<AuditTrail>>,
}

impl KnowledgeBase {
    pub fn new(api_key: &str) -> Result<Self, rusqlite::Error> {
        Self::new_with_params(api_key, "aegis_knowledge.db", QDRANT_URL, false)
    }

    pub fn new_air_gapped(sqlite_path: &str) -> Result<Self, rusqlite::Error> {
        Self::new_with_params("", sqlite_path, QDRANT_URL, true)
    }

    pub fn new_with_params(
        api_key: &str,
        sqlite_path: &str,
        qdrant_url: &str,
        air_gapped: bool,
    ) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(sqlite_path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                source TEXT NOT NULL,
                content TEXT NOT NULL,
                collection TEXT NOT NULL DEFAULT 'osint',
                embedding_id TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                embedding_id TEXT,
                FOREIGN KEY (document_id) REFERENCES documents(id)
            );
            CREATE TABLE IF NOT EXISTS knowledge_items (
                id TEXT PRIMARY KEY,
                content_hash TEXT NOT NULL,
                item_type TEXT NOT NULL,
                content TEXT NOT NULL,
                summary TEXT,
                source TEXT NOT NULL,
                confidence REAL NOT NULL,
                verified_by TEXT NOT NULL,
                tags TEXT NOT NULL,
                related_iocs TEXT NOT NULL,
                first_seen INTEGER NOT NULL,
                last_seen INTEGER NOT NULL,
                embedding_id TEXT,
                version INTEGER NOT NULL DEFAULT 1
            );
            CREATE INDEX IF NOT EXISTS idx_knowledge_items_type_last_seen ON knowledge_items(item_type, last_seen);
        ",
        )?;

        // Migrate older DBs: table may exist without content_hash/version. Do NOT create the unique
        // index until the column exists and legacy rows have unique placeholders (SQLite allows
        // only one row with '' under UNIQUE).
        let _ = conn.execute(
            "ALTER TABLE knowledge_items ADD COLUMN content_hash TEXT NOT NULL DEFAULT ''",
            (),
        );
        let _ = conn.execute(
            "ALTER TABLE knowledge_items ADD COLUMN version INTEGER NOT NULL DEFAULT 1",
            (),
        );
        let _ = conn.execute(
            "UPDATE knowledge_items SET content_hash = ('mig-' || id) WHERE coalesce(trim(content_hash), '') = ''",
            (),
        );
        let _ = conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_knowledge_items_content_hash ON knowledge_items(content_hash)",
            (),
        );

        let _ = conn.execute(
            "ALTER TABLE knowledge_items ADD COLUMN feedback TEXT",
            (),
        );

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;

        let qdrant_client = Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|_| rusqlite::Error::InvalidQuery)?; // fallback error

        // Normalize Qdrant URL: our integration uses REST API (default 6333).
        // If a config points to 6334 (gRPC), fix it to avoid silent failures.
        let mut qdrant_url = qdrant_url.to_string();
        if qdrant_url.ends_with(":6334") {
            qdrant_url = qdrant_url.trim_end_matches(":6334").to_string() + ":6333";
        }

        Ok(Self {
            http_client,
            qdrant_client,
            api_key: api_key.to_string(),
            sqlite_path: sqlite_path.to_string(),
            air_gapped,
            qdrant_url,
            audit: None,
        })
    }

    pub fn with_audit(mut self, audit: Arc<AuditTrail>) -> Self {
        self.audit = Some(audit);
        self
    }

    pub async fn embed_text(&self, text: &str) -> Result<Vec<f64>, String> {
        if self.air_gapped {
            return Err("Air-gapped mode: embeddings are disabled".to_string());
        }
        let request_body = EmbeddingRequest {
            model: "google/gemini-embedding-001".to_string(),
            input: text.to_string(),
        };

        let response = timeout(
            TokioDuration::from_secs(15),
            self.http_client
                .post(EMBEDDING_URL)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send(),
        )
        .await
        .map_err(|_| "Embedding request timed out".to_string())?
        .map_err(|e| format!("HTTP error: {}", e))?;

        let raw_body = timeout(TokioDuration::from_secs(15), response.text())
            .await
            .map_err(|_| "Embedding response read timed out".to_string())?
            .map_err(|e| format!("Failed to read body: {}", e))?;
        let body: EmbeddingResponse = serde_json::from_str(&raw_body)
            .map_err(|e| format!("JSON parse error: {}. Raw: {}", e, raw_body))?;

        if body.data.is_empty() {
            return Err("No embedding data in response".to_string());
        }

        Ok(body.data[0].embedding.clone())
    }

    async fn upsert_to_qdrant(&self, collection: &str, id: &str, vector: Vec<f64>, payload: HashMap<String, String>) -> Result<(), String> {
        if self.air_gapped {
            return Ok(());
        }
        // Ensure collection exists with correct vector dimension.
        self.ensure_qdrant_collection(collection, vector.len()).await?;
        let point = QdrantPoint { id: id.to_string(), vector, payload };
        let upsert = QdrantUpsert { points: vec![point] };
        let url = format!("{}/collections/{}/points?wait=true", self.qdrant_url, collection);
        let resp = timeout(
            TokioDuration::from_secs(15),
            self.qdrant_client.put(&url).json(&upsert).send(),
        )
        .await
        .map_err(|_| "Qdrant upsert timed out".to_string())?
        .map_err(|e| format!("Qdrant upsert error: {}", e))?;
        resp.error_for_status()
            .map_err(|e| format!("Qdrant upsert status error: {}", e))?;
        Ok(())
    }

    async fn ensure_qdrant_collection(&self, name: &str, dim: usize) -> Result<(), String> {
        if self.air_gapped {
            return Ok(());
        }
        if dim == 0 {
            return Err("qdrant collection dimension is 0".into());
        }

        let get_url = format!("{}/collections/{}", self.qdrant_url, name);
        let exists = timeout(TokioDuration::from_secs(8), self.qdrant_client.get(&get_url).send())
            .await
            .map_err(|_| "Qdrant collection check timed out".to_string())?
            .map_err(|e| format!("Qdrant collection check error: {}", e))?
            .status()
            .is_success();
        if exists {
            return Ok(());
        }

        // Create collection (Cosine distance) — minimal config for Phase 1.
        let create_url = format!("{}/collections/{}", self.qdrant_url, name);
        let body = serde_json::json!({
            "vectors": { "size": dim, "distance": "Cosine" }
        });
        let resp = timeout(
            TokioDuration::from_secs(12),
            self.qdrant_client.put(&create_url).json(&body).send(),
        )
        .await
        .map_err(|_| "Qdrant create collection timed out".to_string())?
        .map_err(|e| format!("Qdrant create collection error: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("Qdrant create collection failed: {} {}", status, txt));
        }
        Ok(())
    }

    async fn search_qdrant(&self, collection: &str, vector: Vec<f64>) -> Result<Vec<(String, f64, HashMap<String, String>)>, String> {
        if self.air_gapped {
            return Err("Air-gapped mode: qdrant search disabled".to_string());
        }
        let search = QdrantSearch { vector, limit: TOP_K, with_payload: true };
        let url = format!("{}/collections/{}/points/search", self.qdrant_url, collection);
        let resp = timeout(
            TokioDuration::from_secs(15),
            self.qdrant_client.post(&url).json(&search).send(),
        )
        .await
        .map_err(|_| "Qdrant search timed out".to_string())?
        .map_err(|e| format!("Qdrant search error: {}", e))?;
        let resp = resp
            .error_for_status()
            .map_err(|e| format!("Qdrant search status error: {}", e))?;
        let body: QdrantSearchResponse = timeout(TokioDuration::from_secs(15), resp.json())
            .await
            .map_err(|_| "Qdrant json timed out".to_string())?
            .map_err(|e| format!("Qdrant JSON parse error: {}", e))?;
        Ok(body.result.into_iter().map(|r| (r.id, r.score, r.payload.unwrap_or_default())).collect())
    }

    async fn ingest_to_collection(&self, collection: &str, title: &str, source: &str, content: &str) -> Result<(), String> {
        let doc_id = uuid::Uuid::new_v4().to_string();
        let sqlite_path = self.sqlite_path.clone();
        let doc_id_db = doc_id.clone();
        let title_s = title.to_string();
        let source_s = source.to_string();
        let content_s = content.to_string();
        let collection_s = collection.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            conn.execute(
                "INSERT INTO documents (id, title, source, content, collection) VALUES (?1, ?2, ?3, ?4, ?5)",
                (&doc_id_db, &title_s, &source_s, &content_s, &collection_s),
            )
            .map_err(|e| format!("DB error: {}", e))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))??;

        let chunks = self.chunk_text(content, CHUNK_SIZE);
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_id = uuid::Uuid::new_v4().to_string();
            let vector = if self.air_gapped { vec![] } else { self.embed_text(chunk).await? };
            let mut payload = HashMap::new();
            payload.insert("document_id".to_string(), doc_id.clone());
            payload.insert("title".to_string(), title.to_string());
            payload.insert("source".to_string(), source.to_string());
            payload.insert("chunk_index".to_string(), i.to_string());
            self.upsert_to_qdrant(collection, &chunk_id, vector, payload).await?;
            let sqlite_path = self.sqlite_path.clone();
            let doc_id_s = doc_id.clone();
            let chunk_s = chunk.to_string();
            let chunk_id_s = chunk_id.clone();
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
                conn.execute(
                    "INSERT INTO chunks (id, document_id, chunk_index, content, embedding_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (&chunk_id_s, &doc_id_s, i as i64, &chunk_s, &chunk_id_s),
                )
                .map_err(|e| format!("DB error: {}", e))?;
                Ok(())
            })
            .await
            .map_err(|e| format!("DB join error: {}", e))??;
        }
        Ok(())
    }

    pub async fn ingest_osint(&self, title: &str, source: &str, content: &str) -> Result<(), String> {
        self.ingest_to_collection(COLLECTION_OSINT, title, source, content).await
    }

    pub async fn ingest_darknet(&self, title: &str, source: &str, content: &str) -> Result<(), String> {
        self.ingest_to_collection(COLLECTION_DARKNET, title, source, content).await
    }

    pub async fn search_osint(&self, query: &str) -> Result<Vec<(String, f64, HashMap<String, String>)>, String> {
        if self.air_gapped {
            return self.search_sqlite_like("osint", query).await;
        }
        let vector = self.embed_text(query).await?;
        self.search_qdrant(COLLECTION_OSINT, vector).await
    }

    pub async fn search_darknet(&self, query: &str) -> Result<Vec<(String, f64, HashMap<String, String>)>, String> {
        if self.air_gapped {
            return self.search_sqlite_like("darknet", query).await;
        }
        let vector = self.embed_text(query).await?;
        self.search_qdrant(COLLECTION_DARKNET, vector).await
    }

    async fn search_sqlite_like(&self, collection: &str, query: &str) -> Result<Vec<(String, f64, HashMap<String, String>)>, String> {
        let sqlite_path = self.sqlite_path.clone();
        let collection_s = collection.to_string();
        let q = format!("%{}%", query.to_lowercase());
        let out = tokio::task::spawn_blocking(move || -> Result<Vec<(String, f64, HashMap<String, String>)>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut stmt = conn
                .prepare("SELECT id, title, source FROM documents WHERE lower(collection)=?1 AND (lower(title) LIKE ?2 OR lower(content) LIKE ?2) ORDER BY created_at DESC LIMIT 10")
                .map_err(|e| format!("DB prepare error: {}", e))?;

            let rows = stmt
                .query_map((collection_s, q), |row| {
                    let id: String = row.get(0)?;
                    let title: String = row.get(1)?;
                    let source: String = row.get(2)?;
                    Ok((id, title, source))
                })
                .map_err(|e| format!("DB query error: {}", e))?;

            let mut out = Vec::new();
            for r in rows {
                let (id, title, source) = r.map_err(|e| format!("DB row error: {}", e))?;
                let mut payload = HashMap::new();
                payload.insert("title".to_string(), title);
                payload.insert("source".to_string(), source);
                // score is dummy in offline mode
                out.push((id, 0.5, payload));
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))??;
        Ok(out)
    }

    fn chunk_text(&self, text: &str, max_tokens: usize) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_size = 0;
        for word in words {
            if current_size + word.len() > max_tokens && !current_chunk.is_empty() {
                chunks.push(current_chunk.join(" "));
                current_chunk = Vec::new();
                current_size = 0;
            }
            current_size += word.len() + 1;
            current_chunk.push(word);
        }
        if !current_chunk.is_empty() {
            chunks.push(current_chunk.join(" "));
        }
        chunks
    }

    pub async fn ingest_white(&self, mut item: KnowledgeItem) -> Result<bool, String> {
        item.item_type = KnowledgeType::White;
        self.ingest_item(item, COLLECTION_WHITE, "knowledge_ingested_white").await
    }

    pub async fn ingest_black(&self, mut item: KnowledgeItem) -> Result<bool, String> {
        item.item_type = KnowledgeType::Black;
        self.ingest_item(item, COLLECTION_BLACK, "knowledge_ingested_black").await
    }

    pub async fn search_white(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeItem>, String> {
        self.search_items(query, limit, KnowledgeType::White, COLLECTION_WHITE).await
    }

    pub async fn search_black(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeItem>, String> {
        self.search_items(query, limit, KnowledgeType::Black, COLLECTION_BLACK).await
    }

    pub async fn get_all_white(&self) -> Result<Vec<KnowledgeItem>, String> {
        self.get_all_by_type("white").await
    }

    pub async fn get_all_black(&self) -> Result<Vec<KnowledgeItem>, String> {
        self.get_all_by_type("black").await
    }

    pub async fn get_changed_since(&self, since: i64) -> Result<Vec<KnowledgeItem>, String> {
        let sqlite_path = self.sqlite_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<KnowledgeItem>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, item_type, content, summary, source, confidence, verified_by, tags, related_iocs, first_seen, last_seen, embedding_id, feedback
                     FROM knowledge_items
                     WHERE last_seen > ?1
                     ORDER BY last_seen DESC"
                )
                .map_err(|e| format!("DB prepare error: {}", e))?;
            let rows = stmt
                .query_map([since], knowledge_item_from_row)
                .map_err(|e| format!("DB query error: {}", e))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| format!("DB row error: {}", e))?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    pub async fn get_by_content_hash(&self, hash: &str) -> Result<Option<KnowledgeItem>, String> {
        let sqlite_path = self.sqlite_path.clone();
        let hash_str = hash.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<KnowledgeItem>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, item_type, content, summary, source, confidence, verified_by, tags, related_iocs, first_seen, last_seen, embedding_id, feedback
                     FROM knowledge_items
                     WHERE content_hash = ?1
                     LIMIT 1"
                )
                .map_err(|e| format!("DB prepare error: {}", e))?;
            
            let item = stmt
                .query_row([hash_str], knowledge_item_from_row)
                .optional()
                .map_err(|e| format!("DB query error: {}", e))?;
                
            Ok(item)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    pub async fn get_white_by_id(&self, id: &str) -> Option<KnowledgeItem> {
        self.get_by_id_and_type(id, "white").await
    }

    pub async fn get_black_by_id(&self, id: &str) -> Option<KnowledgeItem> {
        self.get_by_id_and_type(id, "black").await
    }

    async fn get_by_id_and_type(&self, id: &str, item_type: &str) -> Option<KnowledgeItem> {
        let sqlite_path = self.sqlite_path.clone();
        let id_str = id.to_string();
        let type_str = item_type.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<KnowledgeItem>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, item_type, content, summary, source, confidence, verified_by, tags, related_iocs, first_seen, last_seen, embedding_id, feedback
                     FROM knowledge_items
                     WHERE id = ?1 AND item_type = ?2
                     LIMIT 1"
                )
                .map_err(|e| format!("DB prepare error: {}", e))?;
            
            let item = stmt
                .query_row((&id_str, &type_str), knowledge_item_from_row)
                .optional()
                .map_err(|e| format!("DB query error: {}", e))?;
                
            Ok(item)
        })
        .await
        .ok()?
        .unwrap_or(None)
    }

    pub async fn get_last_seen(&self) -> Result<i64, String> {
        let sqlite_path = self.sqlite_path.clone();
        tokio::task::spawn_blocking(move || -> Result<i64, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let last_seen: i64 = conn
                .query_row(
                    "SELECT MAX(last_seen) FROM knowledge_items",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            Ok(last_seen)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    pub async fn get_all_hashes(&self) -> Result<Vec<(String, String)>, String> {
        let sqlite_path = self.sqlite_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<(String, String)>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut stmt = conn.prepare(
                "SELECT id, content_hash FROM knowledge_items WHERE content_hash IS NOT NULL"
            ).map_err(|e| format!("DB prepare error: {}", e))?;
            
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,  // id
                    row.get::<_, String>(1)?,  // content_hash
                ))
            }).map_err(|e| format!("DB query error: {}", e))?;
            
            let mut result = Vec::new();
            for row in rows {
                result.push(row.map_err(|e| format!("DB row error: {}", e))?);
            }
            
            Ok(result)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    async fn get_all_by_type(&self, item_type: &str) -> Result<Vec<KnowledgeItem>, String> {
        let sqlite_path = self.sqlite_path.clone();
        let type_str = item_type.to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<KnowledgeItem>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, item_type, content, summary, source, confidence, verified_by, tags, related_iocs, first_seen, last_seen, embedding_id, feedback
                     FROM knowledge_items
                     WHERE item_type = ?1
                     ORDER BY last_seen DESC"
                )
                .map_err(|e| format!("DB prepare error: {}", e))?;
            let rows = stmt
                .query_map([type_str], knowledge_item_from_row)
                .map_err(|e| format!("DB query error: {}", e))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| format!("DB row error: {}", e))?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    pub async fn count_white(&self) -> Result<usize, String> {
        self.count_by_type("white").await
    }

    pub async fn count_black(&self) -> Result<usize, String> {
        self.count_by_type("black").await
    }

    pub async fn count_black_by_source(&self, source: &str) -> Result<usize, String> {
        let sqlite_path = self.sqlite_path.clone();
        let src = source.to_string();
        tokio::task::spawn_blocking(move || -> Result<usize, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let count: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_items WHERE item_type = 'black' AND source = ?1",
                    [src],
                    |row| row.get(0),
                )
                .map_err(|e| format!("DB query error: {}", e))?;
            Ok(count)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    /// Legacy `documents` table (osint / darknet collections).
    pub async fn count_legacy_documents(&self, collection: &str) -> Result<usize, String> {
        let sqlite_path = self.sqlite_path.clone();
        let col = collection.to_string();
        tokio::task::spawn_blocking(move || -> Result<usize, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let count: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM documents WHERE collection = ?1",
                    [col],
                    |row| row.get(0),
                )
                .map_err(|e| format!("DB query error: {}", e))?;
            Ok(count)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    /// Recent black-knowledge summaries for dashboard (e.g. ФСТЭК БДУ).
    pub async fn recent_black_summaries(&self, limit: usize) -> Result<Vec<String>, String> {
        let items = self.get_all_black().await?;
        Ok(items
            .into_iter()
            .take(limit)
            .map(|i| {
                if let Some(s) = i.summary {
                    format!("{} | {}", i.source, s.chars().take(120).collect::<String>())
                } else {
                    format!("{} | {}", i.source, i.content.chars().take(120).collect::<String>())
                }
            })
            .collect())
    }

    async fn count_by_type(&self, item_type: &str) -> Result<usize, String> {
        let sqlite_path = self.sqlite_path.clone();
        let type_str = item_type.to_string();
        tokio::task::spawn_blocking(move || -> Result<usize, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let count: usize = conn
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_items WHERE item_type = ?1",
                    [type_str],
                    |row| row.get(0),
                )
                .map_err(|e| format!("DB query error: {}", e))?;
            Ok(count)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    /// Human feedback for a stored `knowledge_items` row (by primary id).
    pub async fn set_knowledge_feedback(&self, id: &str, fb: KnowledgeFeedback) -> Result<(), String> {
        let id = id.to_string();
        let s = fb.as_str().to_string();
        let sqlite_path = self.sqlite_path.clone();
        let id_db = id.clone();
        let s_db = s.clone();
        let n = tokio::task::spawn_blocking(move || -> Result<usize, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let n = conn
                .execute("UPDATE knowledge_items SET feedback = ?1 WHERE id = ?2", (&s_db, &id_db))
                .map_err(|e| format!("DB update error: {}", e))?;
            Ok(n)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))??;
        if n == 0 {
            return Err(format!("knowledge id not found: {}", id));
        }
        if let Some(audit) = &self.audit {
            let _ = audit.log_event(
                "knowledge_base",
                &format!("kb_feedback_set id={} feedback={}", &id[..id.len().min(12)], s),
                0.2,
                true,
            );
        }
        Ok(())
    }

    /// Items without human feedback (SQLite `feedback` empty / NULL).
    pub async fn list_pending_feedback(&self, limit: usize) -> Result<Vec<KnowledgeItem>, String> {
        let limit = limit.clamp(1, 100);
        let sqlite_path = self.sqlite_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<KnowledgeItem>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, item_type, content, summary, source, confidence, verified_by, tags, related_iocs, first_seen, last_seen, embedding_id, feedback
                     FROM knowledge_items
                     WHERE feedback IS NULL OR trim(feedback) = ''
                     ORDER BY last_seen DESC LIMIT ?1",
                )
                .map_err(|e| format!("DB prepare error: {}", e))?;
            let rows = stmt
                .query_map([limit as i64], knowledge_item_from_row)
                .map_err(|e| format!("DB query error: {}", e))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| format!("DB row error: {}", e))?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    async fn ingest_item(&self, mut item: KnowledgeItem, qdrant_collection: &str, audit_action: &str) -> Result<bool, String> {
        let now = chrono::Utc::now().timestamp();
        if item.first_seen == 0 { item.first_seen = now; }
        item.last_seen = now;

        // Dedup key: type+source+content (normalized)
        let content_hash = compute_content_hash(&item);
        item.content_hash = content_hash.clone(); // Сохраняем хеш в объекте item для записи в базу!

        // Store metadata in SQLite FIRST (durable), then best-effort embeddings/Qdrant.
        let sqlite_path = self.sqlite_path.clone();
        let row = item.clone();
        let (deduped, canonical_id) = tokio::task::spawn_blocking(move || -> Result<(bool, String), String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            #[derive(Debug)]
            struct Existing {
                id: String,
                version: i64,
                confidence: f64,
                verified_by: String,
                tags: String,
                related_iocs: String,
                first_seen: i64,
            }
            let existing = conn
                .prepare(
                    "SELECT id, version, confidence, verified_by, tags, related_iocs, first_seen
                     FROM knowledge_items WHERE content_hash=?1 LIMIT 1",
                )
                .and_then(|mut st| {
                    st.query_row((&content_hash,), |r| {
                        Ok(Existing {
                            id: r.get(0)?,
                            version: r.get(1)?,
                            confidence: r.get(2)?,
                            verified_by: r.get(3)?,
                            tags: r.get(4)?,
                            related_iocs: r.get(5)?,
                            first_seen: r.get(6)?,
                        })
                    })
                })
                .ok();

            let deduped = existing.is_some();

            let preserved_fb: Option<String> = if let Some(ref ex) = existing {
                conn.query_row(
                    "SELECT feedback FROM knowledge_items WHERE id = ?1",
                    [&ex.id],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
            } else {
                None
            };
            let merged_feedback: Option<String> = match &row.feedback {
                Some(fb) => Some(fb.as_str().to_string()),
                None => preserved_fb,
            };

            let mut id = row.id.clone();
            let mut version: i64 = 1;
            let mut first_seen = row.first_seen;
            let mut confidence = row.confidence;
            let mut verified_by_vec = row.verified_by.clone();
            let mut tags_vec = row.tags.clone();
            let mut iocs_vec = row.related_iocs.clone();

            if let Some(ex) = existing {
                id = ex.id;
                version = ex.version;
                first_seen = ex.first_seen;
                confidence = confidence.max(ex.confidence);

                let ex_verified: Vec<String> = serde_json::from_str(&ex.verified_by).unwrap_or_default();
                let ex_tags: Vec<String> = serde_json::from_str(&ex.tags).unwrap_or_default();
                let ex_iocs: Vec<String> = serde_json::from_str(&ex.related_iocs).unwrap_or_default();
                merge_unique(&mut verified_by_vec, ex_verified);
                merge_unique(&mut tags_vec, ex_tags);
                merge_unique(&mut iocs_vec, ex_iocs);
            }

            let verified_by = serde_json::to_string(&verified_by_vec).map_err(|e| format!("json: {}", e))?;
            let tags = serde_json::to_string(&tags_vec).map_err(|e| format!("json: {}", e))?;
            let iocs = serde_json::to_string(&iocs_vec).map_err(|e| format!("json: {}", e))?;
            let item_type = match row.item_type {
                KnowledgeType::White => "white",
                KnowledgeType::Black => "black",
                KnowledgeType::Hypothesis => "hypothesis",
                KnowledgeType::TTP => "ttp",
            };
            conn.execute(
                "INSERT OR REPLACE INTO knowledge_items
                 (id, content_hash, item_type, content, summary, source, confidence, verified_by, tags, related_iocs, first_seen, last_seen, embedding_id, version, feedback)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                (
                    &id,
                    &content_hash,
                    item_type,
                    &row.content,
                    row.summary.as_deref(),
                    &row.source,
                    confidence,
                    &verified_by,
                    &tags,
                    &iocs,
                    first_seen,
                    row.last_seen,
                    row.embedding_id.as_deref(),
                    version,
                    merged_feedback.as_deref(),
                ),
            )
            .map_err(|e| format!("DB insert error: {}", e))?;
            Ok((deduped, id))
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))??;

        item.id = canonical_id.clone();

        // Embedding + Qdrant (best-effort)
        if !self.air_gapped {
            let embed_input = if let Some(summary) = &item.summary {
                format!("{}\n\n{}", summary, item.content)
            } else {
                item.content.clone()
            };
            match self.embed_text(&embed_input).await {
                Ok(vec) => {
                    item.embedding_id = Some(item.id.clone());
                    let mut payload = HashMap::new();
                    payload.insert("item_type".to_string(), match item.item_type { KnowledgeType::White => "white".into(), KnowledgeType::Black => "black".into(), KnowledgeType::Hypothesis => "hypothesis".into(), KnowledgeType::TTP => "ttp".into() });
                    payload.insert("source".to_string(), item.source.clone());
                    payload.insert("confidence".to_string(), format!("{:.3}", item.confidence));
                    if let Err(e) = self.upsert_to_qdrant(qdrant_collection, &item.id, vec, payload).await {
                        if let Some(audit) = &self.audit {
                            let _ = audit.log_event("knowledge_base", &format!("qdrant_upsert_failed id={} err={}", item.id, e), 0.4, false);
                        }
                    }
                }
                Err(e) => {
                    if let Some(audit) = &self.audit {
                        let _ = audit.log_event("knowledge_base", &format!("embedding_failed id={} err={}", item.id, e), 0.4, false);
                    }
                }
            }
        }

        if let Some(audit) = &self.audit {
            let _ = audit.log_event("knowledge_base", &format!("{} id={} source={} deduped={}", audit_action, item.id, item.source, deduped), 0.25, true);
        }
        Ok(deduped)
    }

    async fn search_items(
        &self,
        query: &str,
        limit: usize,
        kt: KnowledgeType,
        qdrant_collection: &str,
    ) -> Result<Vec<KnowledgeItem>, String> {
        let limit = limit.clamp(1, 50);
        if self.air_gapped {
            return self.search_items_sqlite_like(query, limit, kt).await;
        }

        let vector = self.embed_text(query).await?;
        let hits = self.search_qdrant(qdrant_collection, vector).await?;
        let mut ids: Vec<String> = hits.into_iter().take(limit).map(|(id, _, _)| id).collect();
        ids.dedup();
        self.load_items_by_ids(ids).await
    }

    async fn load_items_by_ids(&self, ids: Vec<String>) -> Result<Vec<KnowledgeItem>, String> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let sqlite_path = self.sqlite_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<KnowledgeItem>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut out = Vec::new();
            for id in ids {
                let item: Option<KnowledgeItem> = conn
                    .query_row(
                        "SELECT id, item_type, content, summary, source, confidence, verified_by, tags, related_iocs, first_seen, last_seen, embedding_id, feedback
                         FROM knowledge_items WHERE id=?1",
                        [&id],
                        knowledge_item_from_row,
                    )
                    .optional()
                    .map_err(|e| format!("DB query error: {}", e))?;
                if let Some(it) = item {
                    out.push(it);
                }
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }

    async fn search_items_sqlite_like(
        &self,
        query: &str,
        limit: usize,
        kt: KnowledgeType,
    ) -> Result<Vec<KnowledgeItem>, String> {
        let sqlite_path = self.sqlite_path.clone();
        let q = format!("%{}%", query.to_lowercase());
        let item_type_s = match kt {
            KnowledgeType::White => "white",
            KnowledgeType::Black => "black",
            KnowledgeType::Hypothesis => "hypothesis",
            KnowledgeType::TTP => "ttp",
        }
        .to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<KnowledgeItem>, String> {
            let conn = Connection::open(&sqlite_path).map_err(|e| format!("DB open error: {}", e))?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, item_type, content, summary, source, confidence, verified_by, tags, related_iocs, first_seen, last_seen, embedding_id, feedback
                     FROM knowledge_items
                     WHERE item_type=?1 AND (lower(content) LIKE ?2 OR lower(ifnull(summary,'')) LIKE ?2)
                     ORDER BY last_seen DESC LIMIT ?3",
                )
                .map_err(|e| format!("DB prepare error: {}", e))?;
            let rows = stmt
                .query_map((&item_type_s, q, limit as i64), knowledge_item_from_row)
                .map_err(|e| format!("DB query error: {}", e))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| format!("DB row error: {}", e))?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("DB join error: {}", e))?
    }
}

fn knowledge_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeItem> {
    let item_type_s: String = row.get(1)?;
    let item_type = match item_type_s.as_str() {
        "white" => KnowledgeType::White,
        "black" => KnowledgeType::Black,
        "hypothesis" => KnowledgeType::Hypothesis,
        "ttp" => KnowledgeType::TTP,
        _ => KnowledgeType::Hypothesis,
    };
    let verified_by_json: String = row.get(6)?;
    let tags_json: String = row.get(7)?;
    let iocs_json: String = row.get(8)?;
    let verified_by: Vec<String> = serde_json::from_str(&verified_by_json).unwrap_or_default();
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    let related_iocs: Vec<String> = serde_json::from_str(&iocs_json).unwrap_or_default();
    let feedback_raw: Option<String> = row.get(12)?;
    Ok(KnowledgeItem {
        id: row.get(0)?,
        item_type,
        content: row.get(2)?,
        summary: row.get(3)?,
        source: row.get(4)?,
        confidence: row.get(5)?,
        verified_by,
        tags,
        related_iocs,
        first_seen: row.get(9)?,
        last_seen: row.get(10)?,
        embedding_id: row.get(11)?,
        content_hash: String::new(),
        feedback: parse_feedback_db(feedback_raw),
    })
}

fn parse_feedback_db(s: Option<String>) -> Option<KnowledgeFeedback> {
    s.as_deref().and_then(KnowledgeFeedback::parse)
}

fn compute_content_hash(item: &KnowledgeItem) -> String {
    let it = match item.item_type {
        KnowledgeType::White => "white",
        KnowledgeType::Black => "black",
        KnowledgeType::Hypothesis => "hypothesis",
        KnowledgeType::TTP => "ttp",
    };
    let mut hasher = Sha256::new();
    hasher.update(it.as_bytes());
    hasher.update(b"\n");
    hasher.update(item.source.trim().as_bytes());
    hasher.update(b"\n");
    hasher.update(item.content.trim().as_bytes());
    hex::encode(hasher.finalize())
}

fn merge_unique(dst: &mut Vec<String>, src: Vec<String>) {
    for v in src {
        if !dst.iter().any(|x| x == &v) {
            dst.push(v);
        }
    }
}