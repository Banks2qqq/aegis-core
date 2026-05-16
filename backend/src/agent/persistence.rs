use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct PersistentStore {
    conn: Arc<Mutex<Connection>>,
}

impl PersistentStore {
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS counters (
                 name TEXT PRIMARY KEY,
                 value INTEGER NOT NULL DEFAULT 0
             );
             INSERT OR IGNORE INTO counters (name, value) VALUES ('threats_blocked', 0);
             INSERT OR IGNORE INTO counters (name, value) VALUES ('osint_count', 0);
             INSERT OR IGNORE INTO counters (name, value) VALUES ('darknet_count', 0);
             INSERT OR IGNORE INTO counters (name, value) VALUES ('active_sentinels', 0);"
        )?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub async fn get(&self, name: &str) -> u64 {
        let conn = self.conn.lock().await;
        conn.query_row(
            "SELECT value FROM counters WHERE name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v as u64)
        .unwrap_or(0)
    }

    pub async fn increment(&self, name: &str) -> u64 {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE counters SET value = value + 1 WHERE name = ?1",
            [name],
        )
        .ok();
        conn.query_row(
            "SELECT value FROM counters WHERE name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v as u64)
        .unwrap_or(0)
    }

    pub async fn set(&self, name: &str, value: u64) {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO counters (name, value) VALUES (?1, ?2)",
            [name, &(value as i64).to_string()],
        )
        .ok();
    }
}