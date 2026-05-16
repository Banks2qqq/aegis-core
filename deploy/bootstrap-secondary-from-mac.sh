#!/usr/bin/env bash
# Bootstrap AEGIS on secondary VPS (Beget node 2).
# Usage:
#   export VPS_HOST=93.189.230.72
#   export VPS_PASSWORD='your-root-password'   # or use SSH key + ssh-mux
#   ./deploy/bootstrap-secondary-from-mac.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST="${VPS_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
export VPS_HOST="$HOST" VPS_USER="$USER"
DEPLOY="${ROOT}/deploy"
OUT="${ROOT}/frontend/out"

# shellcheck source=ssh-mux.sh
source "${DEPLOY}/ssh-mux.sh"
trap ssh_mux_close EXIT

echo "==> Secondary VPS: ${USER}@${HOST}"
if ! ssh_mux_open; then
  echo "SSH failed. Set VPS_PASSWORD or add your SSH key to root@${HOST}" >&2
  exit 1
fi

echo "==> Upload deploy bundle"
ssh_cmd "rm -rf /root/aegis-deploy && mkdir -p /root/aegis-deploy"
for f in nginx-aegis-http-only.conf nginx-aegis-full.conf aegis-agent.env.example \
  aegis-agent.service setup-vps.sh install-nginx-selfsigned.sh; do
  scp_cmd "${DEPLOY}/${f}" "${USER}@${HOST}:/root/aegis-deploy/"
done
scp_cmd "${DEPLOY}/config.secondary.production.yaml" "${USER}@${HOST}:/root/aegis-deploy/config.yaml"
ssh_cmd "chmod +x /root/aegis-deploy/*.sh"

echo "==> Base packages (nginx, ufw, rust…)"
ssh_cmd "/root/aegis-deploy/setup-vps.sh /root/aegis-deploy"
ssh_cmd "cp /root/aegis-deploy/config.yaml /opt/aegis/backend/config.yaml"

echo "==> HTTPS for federation (self-signed on IP until you add a domain)"
ssh_cmd "/root/aegis-deploy/install-nginx-selfsigned.sh 93.189.230.72"

echo "==> Backend tarball + build"
export COPYFILE_DISABLE=1
TAR="/tmp/aegis-backend-secondary-$$.tar.gz"
tar czf "$TAR" -C "$ROOT" --exclude='backend/target' backend proto
scp_cmd "$TAR" "${USER}@${HOST}:/root/aegis-backend.tar.gz"
rm -f "$TAR"
ssh_cmd "mkdir -p /opt/aegis && tar xzf /root/aegis-backend.tar.gz -C /opt/aegis"
ssh_cmd 'bash -lc "source /root/.cargo/env && cd /opt/aegis/backend && cargo build --release --bin agent-cli"'
ssh_cmd "systemctl stop aegis-agent 2>/dev/null || true"
ssh_cmd "cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli && chmod 755 /opt/aegis/bin/agent-cli"

if [[ ! -f /etc/aegis/agent.env ]] 2>/dev/null; then
  :
fi
ssh_cmd "test -f /etc/aegis/agent.env || (cp /root/aegis-deploy/aegis-agent.env.example /etc/aegis/agent.env && chmod 600 /etc/aegis/agent.env)"

echo "==> Copy agent.env from primary (JWT + LLM keys)"
if ssh -o StrictHostKeyChecking=no -o ConnectTimeout=8 root@178.236.16.101 'test -f /etc/aegis/agent.env' 2>/dev/null; then
  scp -o StrictHostKeyChecking=no root@178.236.16.101:/etc/aegis/agent.env /tmp/aegis-agent.env.copy
  scp_cmd /tmp/aegis-agent.env.copy "${USER}@${HOST}:/etc/aegis/agent.env"
  rm -f /tmp/aegis-agent.env.copy
  ssh_cmd "chmod 600 /etc/aegis/agent.env"
  echo "    Copied /etc/aegis/agent.env from primary"
fi

ssh_cmd "systemctl enable aegis-agent && systemctl restart aegis-agent && sleep 2 && systemctl is-active aegis-agent"

if [[ -d "$OUT" ]]; then
  echo "==> Deploy dashboard static (same as primary)"
  tar czf - -C "$OUT" . | ssh_cmd "mkdir -p /var/www/aegis/html && tar xzf - -C /var/www/aegis/html"
else
  echo "==> Skip frontend (no frontend/out — run: cd frontend && npm run build)"
fi

echo ""
echo "Secondary bootstrap done: https://${HOST}/health (self-signed TLS)"
echo "Next:"
echo "  ./deploy/generate-federation-mtls.sh"
echo "  PRIMARY_HOST=178.236.16.101 SECONDARY_HOST=${HOST} ./deploy/distribute-federation-mtls.sh"
echo "  ./deploy/apply-federation-cluster.sh"
