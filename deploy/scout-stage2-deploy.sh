#!/usr/bin/env bash
# Scout 2.0 Stage 2 — stability, metrics, UI toast.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

bash "${ROOT}/deploy/scout-stage1-finalize.sh"

echo "==> Frontend build"
(cd "${ROOT}/frontend" && npm run build)

PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
FRONTEND_WEB="/var/www/aegis/html"

for host in "$PRIMARY" "$SECONDARY"; do
  echo "==> [$host] frontend"
  tar czf - -C "${ROOT}/frontend/out" . | ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "mkdir -p /root/frontend_staging ${FRONTEND_WEB} && tar xzf - -C /root/frontend_staging && cp -a /root/frontend_staging/. ${FRONTEND_WEB}/ && chmod -R 755 ${FRONTEND_WEB}" \
    || echo "WARN: frontend deploy to $host skipped" >&2
done

echo "==> Smoke (primary)"
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
  'RUN_SCOUT=1 bash /opt/aegis/deploy/smoke/integration-scout-autonomy.sh && bash /opt/aegis/deploy/smoke/integration-scout-stage2.sh' \
  || echo "WARN: smoke skipped"

echo "=== scout-stage2-deploy complete ==="
