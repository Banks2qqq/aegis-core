#!/usr/bin/env bash
# C1 — Deploy Talos + FortiGuard scout sources to both VPS nodes.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
BACKEND_REMOTE="/opt/aegis/backend"
BIN_REMOTE="/opt/aegis/bin/agent-cli"

source "${ROOT}/deploy/ssh-mux.sh"

FILES=(
  backend/src/agent/scout_intel/hub.rs
  backend/src/agent/scout_intel/sources/mod.rs
  backend/src/agent/scout_intel/sources/http_util.rs
  backend/src/agent/scout_intel/sources/feed_parse.rs
  backend/src/agent/scout_intel/sources/talos.rs
  backend/src/agent/scout_intel/sources/fortiguard.rs
  backend/src/agent/scout_pipeline.rs
  backend/src/agent/server.rs
)

deploy_host() {
  local host=$1
  export VPS_HOST="$host"
  SSH_CTL="/tmp/aegis-c1-${USER}-${host}.sock"
  export SSH_CONTROL_PATH="$SSH_CTL"
  echo "==> [$host] scout C1"
  ssh_mux_open
  for f in "${FILES[@]}"; do
    scp_cmd "${ROOT}/${f}" "${USER}@${host}:${BACKEND_REMOTE}/${f#backend/}"
  done
  scp_cmd "${ROOT}/deploy/smoke/integration-scout-c1.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/" 2>/dev/null || true
  ssh_cmd "chmod +x /opt/aegis/deploy/smoke/integration-scout-c1.sh 2>/dev/null || true"
  ssh_cmd "bash -lc 'source /root/.cargo/env 2>/dev/null; cd ${BACKEND_REMOTE} && cargo build --release --bin agent-cli && systemctl stop aegis-agent; cp target/release/agent-cli ${BIN_REMOTE}; chmod 755 ${BIN_REMOTE}; systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent'"
  ssh_mux_close
}

bash "${ROOT}/deploy/scout-sync-talos-feed.sh"
bash "${ROOT}/deploy/scout-sync-fortiguard-rss.sh"
deploy_host "$PRIMARY"
deploy_host "$SECONDARY" || echo "WARN: secondary skipped" >&2

echo "==> C1 smoke (fortiguard + talos)"
for host in "$PRIMARY" "$SECONDARY"; do
  url=$([ "$host" = "$SECONDARY" ] && echo https://node2.aegis-security.ru || echo https://aegis-security.ru)
  ssh -o StrictHostKeyChecking=no "root@${host}" \
    "export BASE_URL=${url}; source /etc/aegis/agent.env; export SMOKE_API_KEY=\"\$AEGIS_MONITOR_API_KEY\"; bash /opt/aegis/deploy/smoke/integration-scout-c1.sh" || true
done
echo "=== scout-c1-deploy complete ==="
