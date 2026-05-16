#!/usr/bin/env bash
# Mac only: install full nginx config (fixes /api and /ws after certbot).
set -euo pipefail
[[ "$(uname -s)" == "Darwin" ]] || { echo "Run on Mac"; exit 1; }
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export VPS_HOST="${VPS_HOST:-178.236.16.101}"
export VPS_USER="${VPS_USER:-root}"
source "${ROOT}/deploy/ssh-mux.sh"
trap ssh_mux_close EXIT
ssh_mux_open
scp_cmd "${ROOT}/deploy/nginx-ws-map.conf" "${VPS_USER}@${VPS_HOST}:/etc/nginx/conf.d/aegis-ws-map.conf"
scp_cmd "${ROOT}/deploy/nginx-aegis-full.conf" "${VPS_USER}@${VPS_HOST}:/etc/nginx/sites-available/aegis"
ssh_cmd "ln -sf /etc/nginx/sites-available/aegis /etc/nginx/sites-enabled/aegis; rm -f /etc/nginx/sites-enabled/default; nginx -t && systemctl reload nginx"
ssh_cmd "curl -sS -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/health; echo; systemctl is-active aegis-agent || systemctl restart aegis-agent"
echo "Nginx updated: /api and /ws proxied on HTTPS"
