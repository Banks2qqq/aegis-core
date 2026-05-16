#!/usr/bin/env bash
# Mac: build frontend + backend patch + deploy to VPS
set -euo pipefail
[[ "$(uname -s)" == "Darwin" ]] || { echo "Run on Mac"; exit 1; }
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> Frontend build"
(cd frontend && npm run build)

echo "==> Deploy static site"
bash "$ROOT/deploy_to_vps.sh"

echo "==> Backend + nginx (needs SSH key or password once)"
bash "$ROOT/deploy/continue-build-on-vps.sh"
bash "$ROOT/deploy/fix-nginx-ws.sh" || true

echo "Done. Hard refresh: Cmd+Shift+R on https://aegis-security.ru/dashboard/"
