#!/usr/bin/env bash
# Install Prometheus + Grafana + Uptime Kuma on primary VPS (localhost-only UI).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
USER="${VPS_USER:-root}"
if [[ -z "${MONITOR_API_KEY:-}" ]]; then
  MONITOR_API_KEY=$(ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
    "grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env 2>/dev/null | cut -d= -f2-" || true)
fi
API_KEY="${MONITOR_API_KEY:-${SMOKE_API_KEY:-test-key-enterprise}}"

echo "==> Upload monitoring bundle"
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" "mkdir -p /opt/aegis/monitoring/grafana/provisioning/datasources /etc/aegis/monitoring"
scp -o StrictHostKeyChecking=no -r \
  "${ROOT}/docker-compose.yml" \
  "${ROOT}/prometheus.yml" \
  "${ROOT}/grafana" \
  "${USER}@${PRIMARY}:/opt/aegis/monitoring/"

echo "==> Remote setup (token + docker + compose)"
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" "AEGIS_MONITOR_API_KEY=${API_KEY}" bash -s <<'REMOTE'
set -euo pipefail
API_KEY="${AEGIS_MONITOR_API_KEY:-test-key-enterprise}"

if ! command -v docker >/dev/null; then
  echo "Installing docker..."
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y -qq docker.io docker-compose-v2
  systemctl enable --now docker
fi

TOK=$(curl -sfS -X POST http://127.0.0.1:8080/api/login \
  -H "Content-Type: application/json" \
  -d "{\"api_key\":\"${API_KEY}\"}" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])")
install -d -m 700 /etc/aegis/monitoring
printf '%s' "$TOK" > /etc/aegis/monitoring/bearer_token
chmod 755 /etc/aegis/monitoring
chmod 644 /etc/aegis/monitoring/bearer_token

cd /opt/aegis/monitoring
docker compose pull
docker compose up -d
docker compose ps

cat > /etc/cron.daily/aegis-prometheus-token <<CRON
#!/bin/bash
API_KEY="${API_KEY}"
TOK=\$(curl -sfS -X POST http://127.0.0.1:8080/api/login -H "Content-Type: application/json" \
  -d "{\"api_key\":\"\${API_KEY}\"}" | python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])")
printf '%s' "\$TOK" > /etc/aegis/monitoring/bearer_token
chmod 755 /etc/aegis/monitoring
chmod 644 /etc/aegis/monitoring/bearer_token
CRON
chmod 755 /etc/cron.daily/aegis-prometheus-token
REMOTE

cat <<EOF

Monitoring installed (localhost on primary):
  Prometheus  http://127.0.0.1:9090
  Grafana     http://127.0.0.1:3000
  Uptime Kuma http://127.0.0.1:3001

SSH tunnel: ssh -L 3000:127.0.0.1:3000 -L 9090:127.0.0.1:9090 -L 3001:127.0.0.1:3001 root@${PRIMARY}
Configure Uptime Kuma: /health on both nodes, TCP :8443.
EOF
