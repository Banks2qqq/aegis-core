#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT="$ROOT/frontend/out"
if [[ ! -d "$OUT" ]]; then
  echo "Build output missing: $OUT — run: cd frontend && npm run build" >&2
  exit 1
fi
HOST="${VPS_HOST:-178.236.16.101}"
USER="${VPS_USER:-root}"

ssh_opts=(-o StrictHostKeyChecking=no)
if [[ -n "${SSH_CONTROL_PATH:-}" ]]; then
  ssh_opts+=(-o "ControlPath=${SSH_CONTROL_PATH}")
fi

run_ssh() {
  local remote_cmd="$1"
  if command -v sshpass >/dev/null 2>&1 && [[ -n "${VPS_PASSWORD:-}" ]]; then
    SSHPASS="$VPS_PASSWORD" sshpass -e ssh "${ssh_opts[@]}" "${USER}@${HOST}" "$remote_cmd"
  else
    ssh "${ssh_opts[@]}" "${USER}@${HOST}" "$remote_cmd"
  fi
}

# Use tar stream so nested "out/out" never happens again
echo "==> Upload frontend (tar stream)"
tar czf - -C "$OUT" . | run_ssh "rm -rf /root/frontend_staging /var/www/aegis/html/* && mkdir -p /root/frontend_staging /var/www/aegis/html && tar xzf - -C /root/frontend_staging && cp -a /root/frontend_staging/. /var/www/aegis/html/ && chmod -R 755 /var/www/aegis/html"

echo "==> Verify SCOUT bundle on server"
run_ssh "grep -l SCOUT /var/www/aegis/html/_next/static/chunks/*.js 2>/dev/null | head -1 || echo MISSING_SCOUT"

echo "Deployed static site to ${HOST}:/var/www/aegis/html"
