//! PR5 — HTTP client for federation peer calls (optional mTLS + CA pin).

use std::fs;
use std::path::Path;

use reqwest::Client;

use crate::config::FederationConfig;

pub fn build_federation_http_client(cfg: &FederationConfig) -> Client {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .no_proxy();

    // Outbound: present federation client cert (nginx verifies via ssl_client_certificate).
    // Server TLS uses public PKI (Let's Encrypt) — do not pin federation CA on reqwest.
    if let Some(identity) = load_client_identity(cfg) {
        match identity {
            Ok(id) => builder = builder.identity(id),
            Err(e) => tracing::warn!("Federation mTLS client cert: {}", e),
        }
    }

    match builder.build() {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Federation HTTP client build failed: {e:?}");
            Client::new()
        }
    }
}

fn load_client_identity(cfg: &FederationConfig) -> Option<Result<reqwest::Identity, String>> {
    let cert_path = cfg
        .mtls_client_cert
        .clone()
        .or_else(|| std::env::var("AEGIS_FEDERATION_CLIENT_CERT").ok())
        .filter(|s| !s.is_empty())?;
    let key_path = cfg
        .mtls_client_key
        .clone()
        .or_else(|| std::env::var("AEGIS_FEDERATION_CLIENT_KEY").ok())
        .filter(|s| !s.is_empty())?;
    Some(read_pem_identity(&cert_path, &key_path))
}

fn read_pem_identity(cert_path: &str, key_path: &str) -> Result<reqwest::Identity, String> {
    let cert = fs::read(cert_path).map_err(|e| format!("read cert {}: {}", cert_path, e))?;
    let key = fs::read(key_path).map_err(|e| format!("read key {}: {}", key_path, e))?;
    reqwest::Identity::from_pkcs8_pem(&cert, &key).map_err(|e| format!("identity: {}", e))
}

pub fn mtls_configured(cfg: &FederationConfig) -> bool {
    cfg.mtls_client_cert
        .as_ref()
        .map(|s| Path::new(s).exists())
        .unwrap_or(false)
        || std::env::var("AEGIS_FEDERATION_CLIENT_CERT")
            .ok()
            .filter(|s| !s.is_empty())
            .is_some()
}

pub fn ca_configured(cfg: &FederationConfig) -> bool {
    cfg.mtls_ca_cert
        .as_ref()
        .map(|s| Path::new(s).exists())
        .unwrap_or(false)
        || std::env::var("AEGIS_FEDERATION_CA_CERT")
            .ok()
            .filter(|s| !s.is_empty())
            .is_some()
}
