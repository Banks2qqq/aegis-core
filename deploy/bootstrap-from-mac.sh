#!/usr/bin/env bash
# One-shot deploy after Ubuntu reinstall on Beget (same IP).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST="${VPS_HOST:-178.236.16.101}"
USER="${VPS_USER:-root}"
export VPS_HOST="$HOST" VPS_USER="$USER"
DEPLOY="${ROOT}/deploy"
OUT="${ROOT}/frontend/out"

# shellcheck source=ssh-mux.sh
source "${DEPLOY}/ssh-mux.sh"

trap ssh_mux_close EXIT

echo "==> Open SSH (one password for entire deploy)"
if ! ssh_mux_open; then
  echo "" >&2
  echo "Login failed. Check password (Beget panel). Test: ssh ${USER}@${HOST}" >&2
  exit 1
fi

echo "==> Upload deploy files"
ssh_cmd "rm -rf /root/aegis-deploy && mkdir -p /root/aegis-deploy"
for f in nginx-aegis-http-only.conf nginx-aegis-site.conf config.production.yaml \
  aegis-agent.env.example aegis-agent.service setup-vps.sh; do
  scp_cmd "${DEPLOY}/${f}" "${USER}@${HOST}:/root/aegis-deploy/"
done
ssh_cmd "chmod +x /root/aegis-deploy/setup-vps.sh"

echo "==> Base VPS setup (nginx, firewall, rust…)"
ssh_cmd "/root/aegis-deploy/setup-vps.sh /root/aegis-deploy"

echo "==> Upload backend source"
TAR="/tmp/aegis-backend-$$.tar.gz"
tar czf "$TAR" -C "$ROOT" \
  --exclude='backend/target' \
  --exclude='backend/data/*.db' \
  backend proto
trap 'rm -f "$TAR"; ssh_mux_close' EXIT
scp_cmd "$TAR" "${USER}@${HOST}:/root/aegis-backend.tar.gz"
ssh_cmd "rm -rf /opt/aegis/backend && mkdir -p /opt/aegis && tar xzf /root/aegis-backend.tar.gz -C /opt/aegis"
ssh_cmd "cp /root/aegis-deploy/config.production.yaml /opt/aegis/backend/config.yaml && mkdir -p /opt/aegis/backend/data"

echo "==> Build agent-cli on VPS (~5–15 min)…"
ssh_cmd 'bash -lc "source /root/.cargo/env && cd /opt/aegis/backend && cargo build --release --bin agent-cli"'
ssh_cmd "cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli && chmod 755 /opt/aegis/bin/agent-cli"

echo "==> Start API"
ssh_cmd "systemctl enable aegis-agent && systemctl restart aegis-agent && sleep 2 && systemctl is-active aegis-agent"

if [[ ! -d "$OUT" ]]; then
  echo "==> Build frontend locally"
  (cd "${ROOT}/frontend" && npm run build)
fi

echo "==> Deploy static site"
export SSH_CONTROL_PATH
"${ROOT}/deploy_to_vps.sh"

echo ""
echo "Done. Then on VPS: certbot --nginx -d aegis-security.ru -d www.aegis-security.ru"
echo "Site: https://aegis-security.ru"
