#!/usr/bin/env bash
# PR2 backend + frontend deploy (uses deploy/ssh-mux.sh — no password in repo).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export VPS_HOST="${VPS_HOST:-178.236.16.101}"
export VPS_USER="${VPS_USER:-root}"

# shellcheck source=ssh-mux.sh
source "${ROOT}/deploy/ssh-mux.sh"
trap ssh_mux_close EXIT

PR2_FILES=(
  backend/src/agent/server.rs
  backend/src/agent/main.rs
  backend/src/agent/scout_pipeline.rs
  backend/src/agent/react_service.rs
  backend/src/agent/fusion_engine.rs
  backend/src/agent/persistence.rs
  backend/src/agent/healing_orchestrator.rs
)

ssh_mux_open
echo "==> Upload PR2 sources"
for f in "${PR2_FILES[@]}"; do
  scp_cmd "${ROOT}/${f}" "${VPS_USER}@${VPS_HOST}:/opt/aegis/${f}"
done

echo "==> Build + install binary"
ssh_cmd "bash -lc 'source /root/.cargo/env && cd /opt/aegis/backend && cargo build --release --bin agent-cli'"
ssh_cmd "systemctl stop aegis-agent && cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli && chmod 755 /opt/aegis/bin/agent-cli && systemctl start aegis-agent && sleep 2 && systemctl is-active aegis-agent"

if [[ -d "${ROOT}/frontend/out" ]]; then
  echo "==> Deploy frontend"
  export SSH_CONTROL_PATH
  "${ROOT}/deploy_to_vps.sh"
fi

echo "==> Health"
ssh_cmd "curl -sS http://127.0.0.1:8080/health"
echo ""
echo "PR2 deploy complete"
