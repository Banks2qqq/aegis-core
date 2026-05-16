#!/usr/bin/env bash
# D2 + D3 — finalize pilot 10/10: monitoring verify + runbook checklist.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"

echo "=== pilot-10-finalize ==="

# D2 on primary
ssh -o StrictHostKeyChecking=no "root@${PRIMARY}" 'mkdir -p /opt/aegis/monitoring /opt/aegis/deploy/smoke'
scp -o StrictHostKeyChecking=no \
  "$ROOT/deploy/monitoring/setup-uptime-kuma.sh" \
  "$ROOT/deploy/monitoring/verify-monitoring-d2.sh" \
  "$ROOT/deploy/monitoring/external-probes.sh" \
  "root@${PRIMARY}:/opt/aegis/monitoring/"
scp -o StrictHostKeyChecking=no \
  "$ROOT/deploy/monitoring/aegis-external-probes.service" \
  "$ROOT/deploy/monitoring/aegis-external-probes.timer" \
  "$ROOT/deploy/monitoring/refresh-prometheus-token.sh" \
  "$ROOT/deploy/monitoring/aegis-prometheus-token.service" \
  "$ROOT/deploy/monitoring/aegis-prometheus-token.timer" \
  "root@${PRIMARY}:/opt/aegis/monitoring/"

ssh -o StrictHostKeyChecking=no "root@${PRIMARY}" bash -s <<'REMOTE'
set -euo pipefail
chmod +x /opt/aegis/monitoring/*.sh
apt-get install -y -qq python3-venv python3-pip >/dev/null 2>&1 || true
[[ -f /etc/aegis/uptime-kuma.env ]] && set -a && source /etc/aegis/uptime-kuma.env && set +a
/opt/aegis/monitoring/setup-uptime-kuma.sh
cp /opt/aegis/monitoring/aegis-external-probes.* /etc/systemd/system/
cp /opt/aegis/monitoring/aegis-prometheus-token.* /etc/systemd/system/
chmod 755 /etc/aegis /etc/aegis/monitoring
systemctl daemon-reload
systemctl enable --now aegis-external-probes.timer aegis-prometheus-token.timer
/opt/aegis/monitoring/refresh-prometheus-token.sh
docker restart aegis-prometheus
sleep 35
/opt/aegis/monitoring/verify-monitoring-d2.sh
REMOTE

echo "==> D3 prod smokes (primary + secondary)"
ssh -o StrictHostKeyChecking=no "root@${PRIMARY}" \
  'export BASE_URL=https://aegis-security.ru; source /etc/aegis/agent.env; export SMOKE_API_KEY="$AEGIS_MONITOR_API_KEY"; bash /opt/aegis/deploy/smoke/smoke-prod-vps.sh'
ssh -o StrictHostKeyChecking=no "root@${SECONDARY}" \
  'export BASE_URL=https://node2.aegis-security.ru; export EXPECT_HEAL_APPLY=1; export EXPECT_CONTAIN_ENFORCE=1; source /etc/aegis/agent.env; export SMOKE_API_KEY="$AEGIS_MONITOR_API_KEY"; bash /opt/aegis/deploy/smoke/smoke-prod-vps.sh'

echo "=== pilot-10-finalize: PASS ==="
