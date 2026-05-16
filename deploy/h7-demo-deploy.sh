#!/usr/bin/env bash
# H7 — sync demo E2E smoke and run on both VPS.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

for host in "$PRIMARY" "$SECONDARY"; do
  echo "==> H7 smoke @ ${host}"
  scp -o StrictHostKeyChecking=no \
    "${ROOT}/deploy/smoke/lib.sh" \
    "${ROOT}/deploy/smoke/integration-demo-e2e.sh" \
    "${ROOT}/deploy/smoke/integration-react-status.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "chmod +x /opt/aegis/deploy/smoke/*.sh && \
     source /etc/aegis/agent.env && \
     export BASE_URL=http://127.0.0.1:8080 SMOKE_API_KEY=\"\${AEGIS_MONITOR_API_KEY}\" && \
     /opt/aegis/deploy/smoke/integration-demo-e2e.sh"
done

echo "==> H7 demo E2E: OK on both nodes"
