#!/usr/bin/env bash
# Generate production API keys on primary VPS (idempotent append to agent.env).
set -euo pipefail
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"

ssh -o StrictHostKeyChecking=no "root@${PRIMARY}" bash -s <<'REMOTE'
set -euo pipefail
ENV=/etc/aegis/agent.env
touch "$ENV"
chmod 600 "$ENV"
gen() { openssl rand -hex 24; }
upsert() {
  local k=$1 v=$2
  if grep -q "^${k}=" "$ENV" 2>/dev/null; then return; fi
  echo "${k}=${v}" >> "$ENV"
  echo "added ${k}"
}
upsert AEGIS_MONITOR_API_KEY "$(gen)"
upsert AEGIS_DASHBOARD_API_KEY "$(gen)"
grep -q '^AEGIS_DEV_MODE=' "$ENV" || echo 'AEGIS_DEV_MODE=0' >> "$ENV"
# Alert + prometheus use monitor key
MON=$(grep '^AEGIS_MONITOR_API_KEY=' "$ENV" | cut -d= -f2-)
if [[ -f /etc/aegis/federation-alert.env ]]; then
  grep -v '^API_KEY=' /etc/aegis/federation-alert.env > /tmp/fa.env || true
  echo "API_KEY=${MON}" >> /tmp/fa.env
  chmod 600 /tmp/fa.env && mv /tmp/fa.env /etc/aegis/federation-alert.env
fi
REMOTE

echo "Keys on primary. Rebuild agent + update prometheus cron:"
echo "  ssh root@${PRIMARY} 'grep AEGIS_.*_API_KEY /etc/aegis/agent.env'"
