#!/usr/bin/env bash
# Scout 2.0 Stage 3 — OTX + VirusTotal collectors (both nodes).
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
  SSH_CTL="/tmp/aegis-scout3-${USER}-${host}.sock"
  export SSH_CONTROL_PATH="$SSH_CTL"

  echo "==> [$host] upload scout stage 3"
  ssh_mux_open
  scp_dir_cmd "${ROOT}/backend/src/agent/scout_intel" \
    "${USER}@${host}:${BACKEND_REMOTE}/src/agent/"
  ssh_cmd "bash -lc 'source /root/.cargo/env 2>/dev/null || true; cd ${BACKEND_REMOTE} && cargo build --release --bin agent-cli && systemctl stop aegis-agent; cp target/release/agent-cli ${BIN_REMOTE}; chmod 755 ${BIN_REMOTE}; systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent'"
  ssh_mux_close
}

deploy_host "$PRIMARY"
deploy_host "$SECONDARY" || echo "WARN: secondary skipped" >&2

echo ""
echo "TI keys on primary:"
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
  "grep -E '^(OTX_API_KEY|VT_API_KEY)=' ${BACKEND_REMOTE}/../agent.env /etc/aegis/agent.env 2>/dev/null | awk -F= '{if(length(\$2)>0) print \$1\"=set\"; else print \$1\"=MISSING\"}'" \
  || ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
  "awk -F= '/^(OTX_API_KEY|VT_API_KEY)=/{if(length(\$2)>0) print \$1\"=set\"; else print \$1\"=MISSING\"}' /etc/aegis/agent.env"

echo "=== scout-stage3-deploy complete ==="
echo "If keys missing: create deploy/.scout-ti-keys.env and run deploy/scout-apply-ti-keys.sh"
