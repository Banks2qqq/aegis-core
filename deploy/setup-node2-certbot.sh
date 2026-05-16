#!/usr/bin/env bash
# Run ON secondary VPS (or via ssh) after DNS A record node2 -> this host.
set -euo pipefail
DOMAIN="${1:-node2.aegis-security.ru}"
DEPLOY_DIR="${2:-/root/aegis-deploy}"
EMAIL="${CERTBOT_EMAIL:-}"

echo "==> nginx HTTP for ${DOMAIN}"
cp "${DEPLOY_DIR}/nginx-aegis-node2-http.conf" /etc/nginx/sites-available/aegis
ln -sf /etc/nginx/sites-available/aegis /etc/nginx/sites-enabled/aegis
rm -f /etc/nginx/sites-enabled/default
nginx -t
systemctl reload nginx

echo "==> certbot"
CERTBOT_ARGS=(certonly --nginx -d "${DOMAIN}" --non-interactive --agree-tos)
if [[ -n "$EMAIL" ]]; then
  CERTBOT_ARGS+=(--email "$EMAIL")
else
  CERTBOT_ARGS+=(--register-unsafely-without-email)
fi
certbot "${CERTBOT_ARGS[@]}"

echo "==> nginx HTTPS"
cp "${DEPLOY_DIR}/nginx-aegis-node2-full.conf" /etc/nginx/sites-available/aegis
nginx -t
systemctl reload nginx
echo "[certbot] ${DOMAIN} ready: https://${DOMAIN}/health"
