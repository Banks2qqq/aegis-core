//! P2P Discovery — автоматическое обнаружение нод AEGIS

use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::time::Duration;

use crate::audit::AuditTrail;
use crate::distributed_oracle::ConsensusLayer;

const MULTICAST_ADDR: &str = "224.0.0.1:9999";
const HEARTBEAT_INTERVAL: u64 = 30; // каждые 30 секунд

pub struct P2pDiscovery {
    node_id: String,
    audit: Arc<AuditTrail>,
    consensus: Arc<tokio::sync::Mutex<ConsensusLayer>>,
}

impl P2pDiscovery {
    pub fn new(node_id: &str, audit: Arc<AuditTrail>, consensus: Arc<tokio::sync::Mutex<ConsensusLayer>>) -> Self {
        Self {
            node_id: node_id.to_string(),
            audit,
            consensus,
        }
    }

    /// Запускает фоновое обнаружение нод
    pub async fn start(&self) {
        let node_id = self.node_id.clone();
        let audit = self.audit.clone();
        let consensus = self.consensus.clone();

        tokio::spawn(async move {
            // Присоединяемся к multicast группе
            let socket = match UdpSocket::bind("0.0.0.0:9999").await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("P2P Discovery: failed to bind to 9999 (likely another node is running locally). Falling back to random port: {}", e);
                    match UdpSocket::bind("0.0.0.0:0").await {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!("P2P Discovery: failed to bind fallback UDP: {}", e);
                            return;
                        }
                    }
                }
            };

            if let Err(e) = socket.join_multicast_v4(
                "224.0.0.1".parse().unwrap(),
                "0.0.0.0".parse().unwrap(),
            ) {
                tracing::error!("P2P Discovery: failed to join multicast: {}", e);
                return;
            }

            tracing::info!("P2P Discovery: started on {}", MULTICAST_ADDR);

            let mut buf = [0u8; 1024];
            let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL));

            loop {
                tokio::select! {
                    // Отправляем heartbeat
                    _ = interval.tick() => {
                        let msg = format!("AEGIS_NODE:{}:ALIVE", node_id);
                        if let Err(e) = socket.send_to(msg.as_bytes(), MULTICAST_ADDR).await {
                            tracing::warn!("P2P Discovery: send failed: {}", e);
                        }
                    }

                    // Получаем heartbeat от других нод
                    result = socket.recv_from(&mut buf) => {
                        if let Ok((len, addr)) = result {
                            let msg = String::from_utf8_lossy(&buf[..len]);
                            if msg.starts_with("AEGIS_NODE:") && !msg.contains(&node_id) {
                                let parts: Vec<&str> = msg.split(':').collect();
                                if parts.len() >= 3 {
                                    let remote_node_id = parts[1];
                                    tracing::info!("P2P Discovery: found node {} at {}", remote_node_id, addr);

                                    // Регистрируем ноду в ConsensusLayer
                                    let mut c = consensus.lock().await;
                                    c.register_node(remote_node_id);

                                    let _ = audit.log_event(
                                        "p2p",
                                        &format!("node_discovered id={} addr={}", remote_node_id, addr),
                                        0.1,
                                        true,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
