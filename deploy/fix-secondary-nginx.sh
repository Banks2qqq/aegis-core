#!/usr/bin/env bash
# Run ON secondary VPS as root — fixes nginx (HTTPS + /health + /federation proxy).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
"${SCRIPT_DIR}/install-nginx-selfsigned.sh" "93.189.230.72"
systemctl is-active nginx
curl -k -sfS https://127.0.0.1/health
echo ""
echo "OK: secondary nginx + health"
