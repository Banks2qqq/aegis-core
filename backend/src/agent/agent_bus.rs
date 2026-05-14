use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentType {
    Oracle,
    Sentinel,
    Scout,
    Deceiver,
    Inquisitor,
    ThreatHunter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub from: AgentType,
    pub to: AgentType,
    pub message_type: String,
    pub payload: String,
    pub correlation_id: Option<String>,
    pub timestamp: i64,
}

pub struct AgentSubscription {
    pub agent_type: AgentType,
    pub receiver: broadcast::Receiver<AgentMessage>,
}

pub struct AgentBus {
    sender: broadcast::Sender<AgentMessage>,
    active_agents: Arc<Mutex<HashMap<String, AgentType>>>,
}

impl AgentBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1024); // Enterprise load resilience
        Self {
            sender,
            active_agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, agent_id: &str, agent_type: AgentType) -> AgentSubscription {
        let mut agents = self.active_agents.lock().await;
        agents.insert(agent_id.to_string(), agent_type.clone());
        AgentSubscription {
            agent_type,
            receiver: self.sender.subscribe(),
        }
    }

    pub fn send(&self, msg: AgentMessage) -> Result<usize, broadcast::error::SendError<AgentMessage>> {
        self.sender.send(msg)
    }

    pub async fn send_task(&self, from: AgentType, to: AgentType, task: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let msg = AgentMessage {
            id: id.clone(),
            from,
            to,
            message_type: "task".into(),
            payload: task.to_string(),
            correlation_id: None,
            timestamp: chrono::Utc::now().timestamp(),
        };
        let _ = self.sender.send(msg);
        id
    }

    pub async fn request_response(
        &self,
        from: AgentType,
        to: AgentType,
        query: &str,
        timeout_secs: u64,
    ) -> Option<AgentMessage> {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let msg = AgentMessage {
            id: uuid::Uuid::new_v4().to_string(),
            from,
            to,
            message_type: "query".into(),
            payload: query.to_string(),
            correlation_id: Some(correlation_id.clone()),
            timestamp: chrono::Utc::now().timestamp(),
        };
        let _ = self.sender.send(msg);

        let mut rx = self.sender.subscribe();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

        loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(msg)) => {
                    if msg.correlation_id.as_deref() == Some(&correlation_id) && msg.message_type == "response" {
                        return Some(msg);
                    }
                }
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(broadcast::error::RecvError::Closed)) => return None,
                Err(_) => return None,
            }
        }
    }

    pub fn broadcast_to_type(&self, target_type: AgentType, message_type: &str, payload: &str) {
        let msg = AgentMessage {
            id: uuid::Uuid::new_v4().to_string(),
            from: AgentType::Oracle,
            to: target_type.clone(),
            message_type: message_type.into(),
            payload: payload.to_string(),
            correlation_id: None,
            timestamp: chrono::Utc::now().timestamp(),
        };
        let _ = self.sender.send(msg);
    }

    pub fn alert_all_sentinels(&self, threat_signature: &str) {
        self.broadcast_to_type(AgentType::Sentinel, "new_threat_pattern", threat_signature);
    }

    pub async fn scout_asks_oracle(&self, vulnerability: &str) -> Option<String> {
        let response = self
            .request_response(
                AgentType::Scout,
                AgentType::Oracle,
                &format!("Есть ли похожие уязвимости у других клиентов? Детали: {}", vulnerability),
                10,
            )
            .await;
        response.map(|r| r.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_task() {
        let bus = AgentBus::new();
        let id = bus.send_task(AgentType::Oracle, AgentType::Sentinel, "scan port 443").await;
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn test_alert_all_sentinels() {
        let bus = AgentBus::new();
        bus.alert_all_sentinels("Golden SAML detected on IdP");
    }
}