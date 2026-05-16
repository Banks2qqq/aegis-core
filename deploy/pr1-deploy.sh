#!/usr/bin/env bash
# PR1 finish: honest status/knowledge APIs, federation config, frontend (uses ssh-mux).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export VPS_HOST="${VPS_HOST:-178.236.16.101}"
export VPS_USER="${VPS_USER:-root}"

# shellcheck source=ssh-mux.sh
source "${ROOT}/deploy/ssh-mux.sh"
trap ssh_mux_close EXIT

PR1_FILES=(
  backend/src/agent/server.rs
  backend/src/agent/knowledge.rs
  backend/src/agent/fusion_engine.rs
)

ssh_mux_open
echo "==> Upload PR1 sources + production config"
for f in "${PR1_FILES[@]}"; do
  scp_cmd "${ROOT}/${f}" "${VPS_USER}@${VPS_HOST}:/opt/aegis/${f}"
done
scp_cmd "${ROOT}/deploy/config.production.yaml" "${VPS_USER}@${VPS_HOST}:/opt/aegis/backend/config.yaml"

echo "==> Build + install backend"
ssh_cmd "bash -lc 'source /root/.cargo/env && cd /opt/aegis/backend && cargo build --release --bin agent-cli'"
ssh_cmd "systemctl stop aegis-agent && cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli && chmod 755 /opt/aegis/bin/agent-cli && systemctl start aegis-agent && sleep 2 && systemctl is-active aegis-agent"

if [[ -d "${ROOT}/frontend/out" ]]; then
  echo "==> Deploy frontend"
  export SSH_CONTROL_PATH
  "${ROOT}/deploy_to_vps.sh"
fi

echo "==> Smoke"
ssh_cmd "curl -sS http://127.0.0.1:8080/health"
echo ""
echo "PR1 deploy complete"
