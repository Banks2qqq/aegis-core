#!/usr/bin/env bash
# Deploy /api/heal/smoke + action-plane flags; run staging smokes on secondary.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
BACKEND_REMOTE="/opt/aegis/backend"
BIN_REMOTE="/opt/aegis/bin/agent-cli"

source "${ROOT}/deploy/ssh-mux.sh"

deploy_backend() {
  local host=$1
  export VPS_HOST="$host"
  SSH_CTL="/tmp/aegis-b1-${USER}-${host}.sock"
  export SSH_CONTROL_PATH="$SSH_CTL"
  echo "==> [$host] build agent-cli (heal/smoke endpoint)"
  ssh_mux_open
  scp_cmd "${ROOT}/backend/src/agent/server.rs" \
    "${USER}@${host}:${BACKEND_REMOTE}/src/agent/server.rs"
  ssh_cmd "bash -lc 'source /root/.cargo/env 2>/dev/null; cd ${BACKEND_REMOTE} && cargo build --release --bin agent-cli && systemctl stop aegis-agent; cp target/release/agent-cli ${BIN_REMOTE}; chmod 755 ${BIN_REMOTE}; systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent'"
  ssh_mux_close
}

for f in \
  integration-heal-apply.sh \
  integration-scout-contain.sh \
  smoke-staging-action.sh; do
  scp -o StrictHostKeyChecking=no \
    "${ROOT}/deploy/smoke/${f}" \
    "root@${PRIMARY}:/opt/aegis/deploy/smoke/" 2>/dev/null || true
  scp -o StrictHostKeyChecking=no \
    "${ROOT}/deploy/smoke/${f}" \
    "root@${SECONDARY}:/opt/aegis/deploy/smoke/" 2>/dev/null || true
done

deploy_backend "$PRIMARY"
deploy_backend "$SECONDARY"

bash "${ROOT}/deploy/staging-enable-action-plane.sh"

echo "==> staging smokes on secondary"
ssh -o StrictHostKeyChecking=no "root@${SECONDARY}" \
  'chmod +x /opt/aegis/deploy/smoke/*.sh; export BASE_URL=https://node2.aegis-security.ru; source /etc/aegis/agent.env; export SMOKE_API_KEY="$AEGIS_MONITOR_API_KEY"; bash /opt/aegis/deploy/smoke/smoke-staging-action.sh'

echo "==> prod dry-run heal smoke on primary"
ssh -o StrictHostKeyChecking=no "root@${PRIMARY}" \
  'export BASE_URL=https://aegis-security.ru; source /etc/aegis/agent.env; export SMOKE_API_KEY="$AEGIS_MONITOR_API_KEY"; export EXPECT_HEAL_APPLY=0 EXPECT_CONTAIN_ENFORCE=0; bash /opt/aegis/deploy/smoke/integration-heal-apply.sh'

echo "=== staging-action-plane-deploy complete ==="
