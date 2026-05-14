use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct LlmCache {
    cache: Arc<Mutex<HashMap<String, (String, Instant)>>>,
    ttl: Duration,
}

impl LlmCache {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let cache = self.cache.lock().await;
        if let Some((value, timestamp)) = cache.get(key) {
            if timestamp.elapsed() < self.ttl {
                return Some(value.clone());
            }
        }
        None
    }

    pub async fn set(&self, key: String, value: String) {
        let mut cache = self.cache.lock().await;
        cache.insert(key, (value, Instant::now()));

        // Ограничение размера + очистка просроченных записей
        if cache.len() > 8_000 {
            let now = Instant::now();
            // Удаляем все просроченные + самые старые, пока не станет < 5000
            cache.retain(|_, (_, ts)| now.duration_since(*ts) < self.ttl);
            while cache.len() > 5_000 {
                if let Some(oldest_key) = cache.iter()
                    .min_by_key(|(_, (_, ts))| *ts)
                    .map(|(k, _)| k.clone())
                {
                    cache.remove(&oldest_key);
                } else {
                    break;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_seconds),
        }
    }

    pub async fn check(&self, user_id: &str) -> Result<(), String> {
        let mut reqs = self.requests.lock().await;
        let now = Instant::now();
        let entry = reqs.entry(user_id.to_string()).or_insert_with(Vec::new);
        entry.retain(|t| now - *t < self.window);
        if entry.len() >= self.max_requests {
            Err(format!("Rate limit: {}/{}s", self.max_requests, self.window.as_secs()))
        } else {
            entry.push(now);
            Ok(())
        }
    }
}