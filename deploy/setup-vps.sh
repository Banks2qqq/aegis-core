#!/usr/bin/env bash
# Run ON the VPS as root after Ubuntu reinstall.
set -euo pipefail

DEPLOY_DIR="${1:-/root/aegis-deploy}"
export DEBIAN_FRONTEND=noninteractive

echo "==> Packages (nginx, certbot, build tools, SSH)"
apt-get update -qq
apt-get install -y -qq nginx certbot python3-certbot-nginx ufw curl \
  build-essential pkg-config libssl-dev git openssh-server protobuf-compiler

systemctl enable --now ssh
systemctl enable --now nginx

echo "==> Firewall"
ufw allow OpenSSH
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable || true

echo "==> Directories"
mkdir -p /var/www/aegis/html /opt/aegis/bin /opt/aegis/backend/data /etc/aegis

if [[ ! -f /etc/aegis/agent.env ]]; then
  JWT=$(openssl rand -hex 32)
  cp "${DEPLOY_DIR}/aegis-agent.env.example" /etc/aegis/agent.env
  sed -i "s/CHANGE_ME_openssl_rand_hex_32/${JWT}/" /etc/aegis/agent.env
  chmod 600 /etc/aegis/agent.env
  echo "    Created /etc/aegis/agent.env (add AI_API_KEY / XAI_API_KEY if needed)"
fi

if [[ -f "${DEPLOY_DIR}/config.production.yaml" ]]; then
  cp "${DEPLOY_DIR}/config.production.yaml" /opt/aegis/backend/config.yaml
elif [[ -f "${DEPLOY_DIR}/config.yaml" ]]; then
  cp "${DEPLOY_DIR}/config.yaml" /opt/aegis/backend/config.yaml
else
  echo "Missing config.production.yaml or config.yaml in ${DEPLOY_DIR}" >&2
  exit 1
fi

echo "==> Nginx (HTTP, certbot adds HTTPS later)"
cp "${DEPLOY_DIR}/nginx-aegis-http-only.conf" /etc/nginx/sites-available/aegis
rm -f /etc/nginx/sites-enabled/default
ln -sf /etc/nginx/sites-available/aegis /etc/nginx/sites-enabled/aegis
nginx -t
systemctl reload nginx

echo "==> Rust toolchain (for building agent-cli)"
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
# shellcheck disable=SC1091
source /root/.cargo/env
rustc --version

echo "==> systemd unit (binary installed by bootstrap-from-mac.sh)"
cp "${DEPLOY_DIR}/aegis-agent.service" /etc/systemd/system/aegis-agent.service
systemctl daemon-reload

echo ""
echo "VPS base setup done."
echo "Next (from Mac): ./deploy/bootstrap-from-mac.sh"
echo "Then on VPS: certbot --nginx -d aegis-security.ru -d www.aegis-security.ru"
