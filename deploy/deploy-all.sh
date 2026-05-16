#!/usr/bin/env bash
# PR6.4 — End-to-end deploy PR1–PR6: frontend static + backend source tarball + VPS build/install.
# Run from Mac/Linux with SSH to VPS (password once if using ssh-mux — see bootstrap-from-mac.sh).
#
# Usage:
#   ./deploy/deploy-all.sh                  # full path
#   SKIP_FRONTEND=1 ./deploy/deploy-all.sh
#   SKIP_BACKEND_UPLOAD=1 ./deploy/deploy-all.sh   # frontend only (expects backend already on VPS)
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPLOY="$ROOT/deploy"
HOST="${VPS_HOST:-178.236.16.101}"
USER="${VPS_USER:-root}"

export VPS_HOST="$HOST"
export VPS_USER="$USER"

[[ "${SKIP_FRONTEND:-0}" != "1" ]] && {
  echo "==> PR6 [frontend] npm run build"
  (cd "$ROOT/frontend" && npm ci --no-audit 2>/dev/null || npm install --no-audit)
  (cd "$ROOT/frontend" && npm run build)
}

if [[ "${SKIP_BACKEND_UPLOAD:-0}" != "1" ]]; then
  echo "==> PR6 [backend] local release build (sanity)"
  (cd "$ROOT/backend" && cargo build --release --bin agent-cli)

  # shellcheck source=ssh-mux.sh
  source "${DEPLOY}/ssh-mux.sh"
  trap ssh_mux_close EXIT
  echo "==> PR6 [ssh] open multiplexed session (one password if needed)"
  ssh_mux_open || {
    echo "SSH failed — set VPS_HOST VPS_USER or run bootstrap-from-mac.sh first" >&2
    exit 1
  }

  TAR="/tmp/aegis-backend-pr6-$$.tar.gz"
  export COPYFILE_DISABLE="${COPYFILE_DISABLE:-1}"
  tar czf "$TAR" -C "$ROOT" \
    --exclude='backend/target' \
    backend proto

  echo "==> PR6 [scp] backend tarball"
  scp_cmd "$TAR" "${USER}@${HOST}:/root/aegis-backend-pr6.tar.gz"
  rm -f "$TAR"

  echo "==> PR6 [ssh] extract + build agent-cli on VPS (preserves backend/data)"
  ssh_cmd "mkdir -p /opt/aegis && tar xzf /root/aegis-backend-pr6.tar.gz -C /opt/aegis"
  if [[ "${UPDATE_PROD_CONFIG:-0}" == "1" ]]; then
    scp_cmd "$ROOT/deploy/config.production.yaml" "${USER}@${HOST}:/opt/aegis/backend/config.yaml"
  fi
  ssh_cmd 'bash -lc "source /root/.cargo/env 2>/dev/null || true; cd /opt/aegis/backend && cargo build --release --bin agent-cli"'
  ssh_cmd "systemctl stop aegis-agent 2>/dev/null || true"
  ssh_cmd "cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli && chmod 755 /opt/aegis/bin/agent-cli"
  ssh_cmd "systemctl start aegis-agent && sleep 2 && systemctl is-active aegis-agent"

  ssh_mux_close
  trap - EXIT
fi

[[ "${SKIP_FRONTEND:-0}" != "1" ]] && {
  OUT="$ROOT/frontend/out"
  if [[ -d "$OUT" ]]; then
    echo "==> PR6 [frontend] upload static site"
    SSH_CONTROL_PATH="" bash "$ROOT/deploy_to_vps.sh"
  fi
}

echo ""
echo "=== deploy-all.sh complete ==="
echo "Next: verify https://$HOST/health (via nginx) or curl http://127.0.0.1:8080/health on VPS."
echo "Smoke (with agent running locally): BASE_URL=... ./deploy/smoke/smoke-all.sh"
