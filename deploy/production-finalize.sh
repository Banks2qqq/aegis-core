#!/usr/bin/env bash
# Final production hardening (run after Telegram /revoke + new token in .telegram-token).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "═══ AEGIS Production Finalize ═══"

echo "[1/7] Deploy auth.rs fix + rebuild both nodes"
for f in auth.rs; do
  scp -o StrictHostKeyChecking=no "backend/src/agent/$f" "root@178.236.16.101:/opt/aegis/backend/src/agent/"
  scp -o StrictHostKeyChecking=no "backend/src/agent/$f" "root@93.189.230.72:/opt/aegis/backend/src/agent/"
done
ssh -o StrictHostKeyChecking=no root@178.236.16.101 'source /root/.cargo/env; cd /opt/aegis/backend && cargo build --release --bin agent-cli && cp target/release/agent-cli /opt/aegis/bin/agent-cli && systemctl restart aegis-agent'
ssh -o StrictHostKeyChecking=no root@93.189.230.72 'source /root/.cargo/env; cd /opt/aegis/backend && cargo build --release --bin agent-cli && cp target/release/agent-cli /opt/aegis/bin/agent-cli && systemctl restart aegis-agent'

echo "[2/7] Production API keys (monitor + dashboard)"
bash deploy/production-generate-api-keys.sh

echo "[3/7] Monitoring stack + Grafana dashboard + Uptime Kuma probes"
scp -o StrictHostKeyChecking=no -r deploy/monitoring/grafana root@178.236.16.101:/opt/aegis/monitoring/
scp -o StrictHostKeyChecking=no deploy/monitoring/setup-uptime-kuma.sh root@178.236.16.101:/opt/aegis/monitoring/
ssh -o StrictHostKeyChecking=no root@178.236.16.101 bash -s <<'REMOTE'
set -euo pipefail
mkdir -p /opt/aegis/monitoring/grafana/provisioning/dashboards/json
cp -r /opt/aegis/monitoring/grafana/provisioning/dashboards/json/* /opt/aegis/monitoring/grafana/provisioning/dashboards/json/ 2>/dev/null || true
docker restart aegis-grafana 2>/dev/null || true
chmod +x /opt/aegis/monitoring/setup-uptime-kuma.sh
# Uptime Kuma: use existing password if env exists
if [[ -f /etc/aegis/uptime-kuma.env ]]; then source /etc/aegis/uptime-kuma.env; fi
export UK_USER="${UK_USER:-aegis}"
export UK_PASS="${UK_PASS:-}"
if [[ -z "$UK_PASS" ]]; then UK_PASS=$(openssl rand -hex 12); fi
UK_URL=http://127.0.0.1:3001 UK_USER="$UK_USER" UK_PASS="$UK_PASS" /opt/aegis/monitoring/setup-uptime-kuma.sh || echo "WARN: Uptime Kuma API setup — configure UI once at :3001"
REMOTE

echo "[4/7] Prometheus token with MONITOR key"
bash deploy/monitoring/install-monitoring-primary.sh

echo "[5/7] Telegram (requires deploy/federation-alert/.telegram-token)"
if [[ -f deploy/federation-alert/.telegram-token ]]; then
  bash deploy/federation-alert/apply-telegram-token.sh
else
  echo "SKIP Telegram: create deploy/federation-alert/.telegram-token with new BotFather token"
fi

echo "[6/7] Weekly prod smoke cron"
ssh -o StrictHostKeyChecking=no root@178.236.16.101 'cat > /etc/cron.weekly/aegis-federation-smoke <<"CRON"
#!/bin/bash
/opt/aegis/deploy/smoke/integration-federation-prod-vps.sh >>/var/log/aegis-weekly-smoke.log 2>&1 || logger -t aegis-smoke "WEEKLY SMOKE FAILED"
CRON
chmod 755 /etc/cron.weekly/aegis-federation-smoke'

echo "[7/7] Smoke gate"
ssh -o StrictHostKeyChecking=no root@178.236.16.101 /opt/aegis/deploy/smoke/integration-federation-prod-vps.sh

echo "═══ Finalize complete ═══"
