//! H2 — Real deception listeners via Docker (nginx), honest 10/10 branch A.
//!
//! Env:
//!   AEGIS_DECEPTION_RUNTIME=docker|off|memory  (default: docker if available)
//!   AEGIS_DECEPTION_IMAGE=nginx:alpine

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use crate::honeypot_manager::{DynamicDeceptionEngine, HoneypotType};

#[derive(Debug, Clone)]
pub struct DeceptionListener {
    pub runtime: String,
    pub container_name: String,
    pub port: u16,
    pub canary: String,
}

pub struct DeceptionRuntime {
    data_root: PathBuf,
    image: String,
    use_docker: bool,
}

impl DeceptionRuntime {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        let mode = std::env::var("AEGIS_DECEPTION_RUNTIME")
            .unwrap_or_else(|_| "docker".into())
            .to_lowercase();
        let use_docker = !matches!(mode.as_str(), "off" | "memory" | "0" | "false");
        Self {
            data_root: data_root.as_ref().to_path_buf(),
            image: std::env::var("AEGIS_DECEPTION_IMAGE").unwrap_or_else(|_| "nginx:alpine".into()),
            use_docker,
        }
    }

    pub async fn spawn_listener(
        &self,
        honeypot_id: &str,
        htype: &HoneypotType,
        port: u16,
    ) -> Result<DeceptionListener, String> {
        let canary = DynamicDeceptionEngine::generate_canary_token();
        let html = embed_canary(&DynamicDeceptionEngine::generate_fake_content(htype), &canary);

        if self.use_docker && docker_available().await {
            match self
                .spawn_docker(honeypot_id, port, &html, &canary)
                .await
            {
                Ok(listener) => {
                    crate::metrics::record_deception_listener("docker", "pass");
                    return Ok(listener);
                }
                Err(e) => {
                    tracing::warn!("DeceptionRuntime: docker spawn failed: {} — memory fallback", e);
                    crate::metrics::record_deception_listener("docker", "fail");
                }
            }
        }

        crate::metrics::record_deception_listener("memory", "pass");
        Ok(DeceptionListener {
            runtime: "memory".into(),
            container_name: String::new(),
            port,
            canary,
        })
    }

    pub async fn verify_local(&self, port: u16, canary: &str) -> bool {
        let url = format!("http://127.0.0.1:{}/", port);
        let Ok(Ok(out)) = timeout(Duration::from_secs(5), Command::new("curl").args(["-sf", &url]).output()).await
        else {
            return false;
        };
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains(canary)
    }

    async fn spawn_docker(
        &self,
        honeypot_id: &str,
        port: u16,
        html: &str,
        canary: &str,
    ) -> Result<DeceptionListener, String> {
        let work = self
            .data_root
            .join("deception")
            .join(sanitize_id(honeypot_id));
        tokio::fs::create_dir_all(&work)
            .await
            .map_err(|e| format!("mkdir deception work: {}", e))?;
        tokio::fs::write(work.join("index.html"), html)
            .await
            .map_err(|e| format!("write index.html: {}", e))?;

        let cname = format!("aegis-deception-{}", &sanitize_id(honeypot_id)[..12.min(sanitize_id(honeypot_id).len())]);
        let work_s = work.display().to_string();

        let _ = Command::new("docker")
            .args(["rm", "-f", &cname])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        let _ = Command::new("sh")
            .args([
                "-c",
                &format!(
                    "docker ps -q --filter publish=127.0.0.1:{} | xargs -r docker rm -f 2>/dev/null || true",
                    port
                ),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        let port_bind = format!("127.0.0.1:{}:80", port);
        let mut cmd = Command::new("docker");
        cmd.args([
            "run",
            "-d",
            "--name",
            &cname,
            "-p",
            &port_bind,
            "-v",
            &format!("{}:/usr/share/nginx/html:ro", work_s),
            "--memory",
            "64m",
            "--cpus",
            "0.5",
            &self.image,
        ]);
        let out = timeout(Duration::from_secs(60), cmd.output())
            .await
            .map_err(|_| "docker run timeout".to_string())?
            .map_err(|e| format!("docker run: {}", e))?;
        if !out.status.success() {
            return Err(format!(
                "docker run failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let container_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if container_id.is_empty() {
            return Err("docker run returned empty container id".into());
        }

        tokio::time::sleep(Duration::from_millis(800)).await;
        if !self.verify_local(port, canary).await {
            let _ = Command::new("docker")
                .args(["rm", "-f", &cname])
                .status()
                .await;
            return Err("listener up but canary not in HTTP response".into());
        }

        tracing::info!(
            "DeceptionRuntime: docker listener id={} container={} port={} canary={}",
            honeypot_id, cname, port, canary
        );

        Ok(DeceptionListener {
            runtime: "docker".into(),
            container_name: cname,
            port,
            canary: canary.to_string(),
        })
    }
}

fn embed_canary(html: &str, canary: &str) -> String {
    if html.contains(canary) {
        return html.to_string();
    }
    format!("{html}\n<!-- {canary} -->\n")
}

async fn docker_available() -> bool {
    Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
