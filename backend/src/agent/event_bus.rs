use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct Event {
    pub id: String,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
}

#[derive(Clone)]
pub struct EventBus {
    events: Arc<Mutex<VecDeque<Event>>>,
    max_size: usize,
}

impl EventBus {
    pub fn new(max_size: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
        }
    }

    pub async fn publish(&self, event_type: &str, payload: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let event = Event {
            id: id.clone(),
            event_type: event_type.to_string(),
            payload: payload.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        let mut events = self.events.lock().await;
        events.push_back(event);
        if events.len() > self.max_size {
            events.pop_front();
        }
        id
    }

    pub async fn get_since(&self, timestamp: i64) -> Vec<Event> {
        let events = self.events.lock().await;
        events.iter().filter(|e| e.timestamp >= timestamp).cloned().collect()
    }
}