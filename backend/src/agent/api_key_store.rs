//! H5 — API keys stored as SHA-256(pepper || plaintext); never persist plaintext.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyIdentity {
    pub sub: String,
    pub tier: String,
    pub label: String,
    pub scopes: Vec<String>,
}

pub fn hash_api_key(plaintext: &str, pepper: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pepper);
    hasher.update(plaintext.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS api_keys (
            key_hash TEXT PRIMARY KEY,
            sub TEXT NOT NULL,
            tier TEXT NOT NULL,
            label TEXT NOT NULL,
            scopes_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT 'manual'
        );
        CREATE INDEX IF NOT EXISTS idx_api_keys_sub ON api_keys(sub);
        CREATE TABLE IF NOT EXISTS auth_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| format!("api_keys schema: {}", e))
}

pub fn upsert_key(
    conn: &Connection,
    plaintext: &str,
    pepper: &[u8],
    sub: &str,
    tier: &str,
    label: &str,
    scopes: &[String],
    source: &str,
) -> Result<String, String> {
    let hash = hash_api_key(plaintext, pepper);
    let scopes_json =
        serde_json::to_string(scopes).map_err(|e| format!("scopes json: {}", e))?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO api_keys (key_hash, sub, tier, label, scopes_json, created_at, enabled, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)
         ON CONFLICT(key_hash) DO UPDATE SET
           sub=excluded.sub, tier=excluded.tier, label=excluded.label,
           scopes_json=excluded.scopes_json, enabled=1, source=excluded.source",
        (
            &hash,
            sub,
            tier,
            label,
            &scopes_json,
            now,
            source,
        ),
    )
    .map_err(|e| format!("upsert api_key: {}", e))?;
    Ok(hash)
}

pub fn lookup_identity(
    db_path: &str,
    plaintext: &str,
    pepper: &[u8],
) -> Result<Option<ApiKeyIdentity>, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("auth db open: {}", e))?;
    init_schema(&conn)?;
    let hash = hash_api_key(plaintext, pepper);
    let mut stmt = conn
        .prepare(
            "SELECT sub, tier, label, scopes_json FROM api_keys
             WHERE key_hash = ?1 AND enabled = 1 LIMIT 1",
        )
        .map_err(|e| format!("prepare: {}", e))?;
    let mut rows = stmt
        .query([&hash])
        .map_err(|e| format!("query: {}", e))?;
    let row = match rows.next().map_err(|e| format!("row: {}", e))? {
        Some(r) => r,
        None => return Ok(None),
    };
    let sub: String = row.get(0).map_err(|e| e.to_string())?;
    let tier: String = row.get(1).map_err(|e| e.to_string())?;
    let label: String = row.get(2).map_err(|e| e.to_string())?;
    let scopes_json: String = row.get(3).map_err(|e| e.to_string())?;
    let scopes: Vec<String> = serde_json::from_str(&scopes_json).unwrap_or_else(|_| {
        vec!["read".to_string(), "threats".to_string()]
    });
    Ok(Some(ApiKeyIdentity {
        sub,
        tier,
        label,
        scopes,
    }))
}

fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare("SELECT value FROM auth_meta WHERE key = ?1 LIMIT 1")
        .map_err(|e| format!("meta prepare: {}", e))?;
    let mut rows = stmt
        .query([key])
        .map_err(|e| format!("meta query: {}", e))?;
    if let Some(row) = rows.next().map_err(|e| format!("meta row: {}", e))? {
        return Ok(row.get(0).ok());
    }
    Ok(None)
}

fn meta_set(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO auth_meta (key, value) VALUES (?1, ?2)",
        (key, value),
    )
    .map_err(|e| format!("meta set: {}", e))?;
    Ok(())
}

/// One-time import from AEGIS_*_API_KEY env into hashed rows (idempotent).
pub fn migrate_env_keys_once(db_path: &str, pepper: &[u8]) -> Result<usize, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("auth db open: {}", e))?;
    init_schema(&conn)?;
    if meta_get(&conn, "env_keys_migrated")?.as_deref() == Some("1") {
        return Ok(0);
    }

    let mut count = 0usize;
    let default_scopes = vec!["read".to_string(), "threats".to_string()];

    if let Ok(monitor) = std::env::var("AEGIS_MONITOR_API_KEY") {
        let k = monitor.trim();
        if !k.is_empty() {
            let mut scopes = default_scopes.clone();
            scopes.push("monitor".to_string());
            upsert_key(
                &conn,
                k,
                pepper,
                "monitor",
                "monitor",
                "env-monitor",
                &scopes,
                "env_migrate",
            )?;
            count += 1;
        }
    }

    if let Ok(dashboard) = std::env::var("AEGIS_DASHBOARD_API_KEY") {
        let k = dashboard.trim();
        if !k.is_empty() {
            let mut scopes = default_scopes.clone();
            scopes.push("enterprise".to_string());
            upsert_key(
                &conn,
                k,
                pepper,
                "dashboard",
                "enterprise",
                "env-dashboard",
                &scopes,
                "env_migrate",
            )?;
            count += 1;
        }
    }

    meta_set(&conn, "env_keys_migrated", "1")?;
    if count > 0 {
        tracing::info!(
            "api_key_store: migrated {} env key(s) to hashed api_keys table",
            count
        );
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_lookup_works() {
        let dir = std::env::temp_dir().join(format!("aegis-auth-test-{}", uuid::Uuid::new_v4()));
        let db = dir.join("auth.db");
        let pepper = b"test-pepper";
        let conn = Connection::open(&db).unwrap();
        init_schema(&conn).unwrap();
        upsert_key(
            &conn,
            "secret-key-abc",
            pepper,
            "u1",
            "monitor",
            "test",
            &["read".to_string()],
            "test",
        )
        .unwrap();
        let id = lookup_identity(db.to_str().unwrap(), "secret-key-abc", pepper)
            .unwrap()
            .unwrap();
        assert_eq!(id.sub, "u1");
        assert!(lookup_identity(db.to_str().unwrap(), "wrong", pepper)
            .unwrap()
            .is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
