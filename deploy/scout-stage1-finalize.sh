#!/usr/bin/env bash
# Scout 2.0 Stage 1 finalize + Stage 2 stability — deploy to both VPS.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
BACKEND_REMOTE="/opt/aegis/backend"
BIN_REMOTE="/opt/aegis/bin/agent-cli"

# shellcheck source=ssh-mux.sh
source "${ROOT}/deploy/ssh-mux.sh"

FILES=(
  backend/src/agent/server.rs
  backend/src/agent/metrics.rs
  backend/src/agent/scout_pipeline.rs
  backend/src/agent/scout_orchestrator.rs
)

deploy_host() {
  local host=$1
  export VPS_HOST="$host"
  SSH_CTL="/tmp/aegis-scout1-${USER}-${host}.sock"
  export SSH_CONTROL_PATH="$SSH_CTL"

  echo "==> [$host] scout stage1 finalize"
  ssh_mux_open
  for f in "${FILES[@]}"; do
    scp_cmd "${ROOT}/${f}" "${USER}@${host}:${BACKEND_REMOTE}/${f#backend/}"
  done
  scp_dir_cmd "${ROOT}/backend/src/agent/scout_intel" \
    "${USER}@${host}:${BACKEND_REMOTE}/src/agent/"
  scp_cmd "${ROOT}/deploy/smoke/integration-scout-autonomy.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh_cmd "chmod +x /opt/aegis/deploy/smoke/integration-scout-autonomy.sh"
  ssh_cmd "bash -lc 'source /root/.cargo/env 2>/dev/null; cd ${BACKEND_REMOTE} && cargo build --release --bin agent-cli && systemctl stop aegis-agent; cp target/release/agent-cli ${BIN_REMOTE}; chmod 755 ${BIN_REMOTE}; systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent'"
  ssh_mux_close
}

deploy_host "$PRIMARY"
deploy_host "$SECONDARY" || echo "WARN: secondary skipped" >&2
echo "=== scout-stage1-finalize complete ==="
