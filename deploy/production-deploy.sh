#!/usr/bin/env bash
# Production deploy: build on Linux VPS, frontend, alerts, enable autostart, smoke gate.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  AEGIS Production Deploy                                 ║"
echo "╚══════════════════════════════════════════════════════════╝"

bash "${ROOT}/deploy/federation-pilot-deploy.sh"

echo "==> Enable autostart (both nodes)"
bash "${ROOT}/deploy/production-enable-services.sh"

if [[ "${SKIP_MONITORING:-0}" != "1" ]]; then
  if bash "${ROOT}/deploy/monitoring/install-monitoring-primary.sh"; then
    echo "Monitoring stack OK"
  else
    echo "WARN: monitoring install failed (docker missing?) — set SKIP_MONITORING=1 to skip" >&2
  fi
fi

echo "==> Install secondary federation alert"
bash "${ROOT}/deploy/federation-alert/install-alert-secondary.sh" || echo "WARN: secondary alert skipped" >&2

echo "==> Production smoke gate (primary VPS)"
ssh -o StrictHostKeyChecking=no "${VPS_USER:-root}@${PRIMARY_HOST:-178.236.16.101}" \
  /opt/aegis/deploy/smoke/integration-federation-prod-vps.sh

echo ""
echo "=== Production deploy PASSED ==="
echo "Next: rotate Telegram token if exposed; add dedicated MONITOR_API_KEY in agent.env"
