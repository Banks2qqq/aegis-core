#!/usr/bin/env bash
# Deploy Raft auto-recovery fix (distributed_oracle.rs) to both VPS nodes.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
BACKEND_REMOTE="/opt/aegis/backend"
BIN_REMOTE="/opt/aegis/bin/agent-cli"

# shellcheck source=ssh-mux.sh
source "${ROOT}/deploy/ssh-mux.sh"

deploy_host() {
  local host=$1
  export VPS_HOST="$host"
  SSH_CTL="/tmp/aegis-raft-${USER}-${host}.sock"
  export SSH_CONTROL_PATH="$SSH_CTL"

  echo "==> [$host] raft recovery deploy"
  ssh_mux_open
  scp_cmd "${ROOT}/backend/src/agent/distributed_oracle.rs" \
    "${USER}@${host}:${BACKEND_REMOTE}/src/agent/distributed_oracle.rs"
  scp_cmd "${ROOT}/deploy/smoke/integration-raft-recovery.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/" 2>/dev/null || true
  ssh_cmd "chmod +x /opt/aegis/deploy/smoke/integration-raft-recovery.sh 2>/dev/null || true"
  ssh_cmd "bash -lc 'source /root/.cargo/env 2>/dev/null; cd ${BACKEND_REMOTE} && cargo build --release --bin agent-cli && systemctl stop aegis-agent; cp target/release/agent-cli ${BIN_REMOTE}; chmod 755 ${BIN_REMOTE}; systemctl start aegis-agent; sleep 3; systemctl is-active aegis-agent'"
  ssh_mux_close
}

deploy_host "$PRIMARY"
deploy_host "$SECONDARY" || echo "WARN: secondary skipped" >&2
echo "=== raft-recovery-deploy complete ==="
