use std::fs;
use std::error::Error;
use tonic::transport::{ClientTlsConfig, ServerTlsConfig, Identity, Certificate};

#[derive(Clone)]
pub struct TlsConfig {
    pub server: ServerTlsConfig,
    #[allow(dead_code)]
    pub client: ClientTlsConfig,
}

/// Загружает mTLS-конфигурацию из файлов сертификатов
pub fn load_tls_config(
    ca_cert_path: &str,
    server_cert_path: &str,
    server_key_path: &str,
    client_cert_path: &str,
    client_key_path: &str,
    server_domain: &str,
) -> std::result::Result<TlsConfig, Box<dyn Error + Send + Sync>> {
    let ca_pem = fs::read_to_string(ca_cert_path)?;
    let ca_cert = Certificate::from_pem(ca_pem);

    let server_cert = fs::read_to_string(server_cert_path)?;
    let server_key = fs::read_to_string(server_key_path)?;
    let server_identity = Identity::from_pem(server_cert, server_key);

    let client_cert = fs::read_to_string(client_cert_path)?;
    let client_key = fs::read_to_string(client_key_path)?;
    let client_identity = Identity::from_pem(client_cert, client_key);

    let server_tls = ServerTlsConfig::new()
        .identity(server_identity)
        .client_ca_root(ca_cert.clone());

    let client_tls = ClientTlsConfig::new()
        .domain_name(server_domain)
        .ca_certificate(ca_cert)
        .identity(client_identity);

    Ok(TlsConfig { server: server_tls, client: client_tls })
}

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::net::SocketAddr;

/// Используется маршрутами Federation в `server.rs`; в бинаре `oracle-brain` модуль подключается без HTTP-слоя.
#[allow(dead_code)]
pub async fn mtls_auth_layer(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Получаем информацию о TLS соединении
    if let Some(tls_info) = request.extensions().get::<axum::extract::ConnectInfo<SocketAddr>>() {
        // В реальной реализации здесь проверяем клиентский сертификат
        // Пока просто логируем и пропускаем (заглушка для теста)
        tracing::debug!("mTLS connection from {:?}", tls_info);
    }

    // TODO: Добавить реальную проверку сертификата
    // if !verify_client_cert(&request) { return Err(StatusCode::UNAUTHORIZED); }

    Ok(next.run(request).await)
}