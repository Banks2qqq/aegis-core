use tonic::{transport::Server, Request, Response, Status};
use tokio::sync::{mpsc, broadcast};
use tokio_stream::wrappers::ReceiverStream;
use std::pin::Pin;
use tokio_stream::Stream;
use aegis::sentinel_oracle_server::{SentinelOracle, SentinelOracleServer};
use aegis::{Alert, SubscribeRequest, HealthRequest, EventRequest, HealthResponse, OracleDecision};

#[path = "../agent/mtls.rs"]
mod mtls;

pub mod aegis {
    tonic::include_proto!("aegis");
}

#[derive(Debug)]
pub struct MyOracle {
    inner_tx: broadcast::Sender<Alert>,
}

#[tonic::async_trait]
impl SentinelOracle for MyOracle {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<Alert, Status>> + Send>>;

    async fn subscribe(&self, _request: Request<SubscribeRequest>) -> Result<Response<Self::SubscribeStream>, Status> {
        println!(">>> [NEW SUBSCRIPTION]: Агент подключился к защитному контуру.");
        let mut rx = self.inner_tx.subscribe();
        let (tx, out_rx) = mpsc::channel(100);

        tokio::spawn(async move {
            while let Ok(alert) = rx.recv().await {
                if tx.send(Ok(alert)).await.is_err() { break; }
            }
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(out_rx))))
    }

    async fn report_event(&self, request: Request<EventRequest>) -> Result<Response<OracleDecision>, Status> {
        let event = request.into_inner();
        
        // 1. АНАЛИЗ УГРОЗЫ: Ищем подозрительные слова в поле action
        let action_lower = event.action.to_lowercase();
        let is_suspicious = action_lower.contains("critical") || 
                            action_lower.contains("attack") || 
                            action_lower.contains("unauthorized") ||
                            action_lower.contains("error");

        // 2. ВЫЧИСЛЕНИЕ РИСКА (ИСПРАВЛЕНО НА f64)
        let calculated_risk: f64 = if is_suspicious { 0.95 } else { 0.1 };
        
        // 3. РАССЫЛКА: Если риск высокий, отправляем алерт Агенту
        if is_suspicious {
            let alert = Alert {
                message: format!("КРИТИЧЕСКОЕ СОБЫТИЕ: '{}' на ресурсе {}", event.action, event.resource_arn),
                severity: calculated_risk, 
                timestamp: event.timestamp.clone(),
            };
            println!("!!! [ВНИМАНИЕ]: Обнаружена угроза ({}). Рассылка алертов...", event.action);
            let _ = self.inner_tx.send(alert);
        }

        // 4. ВЕРДИКТ ДЛЯ SENTINEL
        Ok(Response::new(OracleDecision { 
            decision_id: event.event_id, 
            verdict: if is_suspicious { 2 } else { 1 }, 
            risk_score: calculated_risk,
            processing_time_ms: 2,
            reason: format!("Action '{}' evaluated", event.action),
        }))
    }

    async fn health_check(&self, _request: Request<HealthRequest>) -> Result<Response<HealthResponse>, Status> {
        Ok(Response::new(HealthResponse { 
            oracle_alive: true, 
            active_sentinels: 1 
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Канал для связи Оракула и Агентов
    let (tx, _) = broadcast::channel(100);
    let addr = "127.0.0.1:9090".parse()?;
    
    println!("\n[V6.0] AEGIS ORACLE: BOOTED IN MONITORING MODE");
    println!("Ожидаю отчеты от Sentinel на порту 9090...");

    let oracle = MyOracle { inner_tx: tx };

    let use_mtls = std::env::var("AEGIS_MTLS").map(|v| v == "1").unwrap_or(false);
    let mut builder = Server::builder();
    if use_mtls {
        let ca = std::env::var("AEGIS_MTLS_CA_CERT")?;
        let server_cert = std::env::var("AEGIS_MTLS_ORACLE_CERT")?;
        let server_key = std::env::var("AEGIS_MTLS_ORACLE_KEY")?;
        let client_cert = std::env::var("AEGIS_MTLS_AGENT_CERT")?;
        let client_key = std::env::var("AEGIS_MTLS_AGENT_KEY")?;
        let domain = std::env::var("AEGIS_MTLS_ORACLE_DOMAIN").unwrap_or_else(|_| "oracle.local".to_string());

        let tls = mtls::load_tls_config(&ca, &server_cert, &server_key, &client_cert, &client_key, &domain)?;
        builder = builder.tls_config(tls.server)?;
        println!("[mTLS] ENABLED for Sentinel ↔ Oracle gRPC");
    } else {
        println!("[mTLS] DISABLED (set AEGIS_MTLS=1 to enforce)");
    }

    builder
        .add_service(SentinelOracleServer::new(oracle))
        .serve(addr)
        .await?;

    Ok(())
}