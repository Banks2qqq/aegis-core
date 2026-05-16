#!/usr/bin/env bash
# Pilot final deploy: local build → both VPS backend → frontend → monitoring → smokes.
# Run from Mac: ./deploy/pilot-final-deploy.sh
# Skip steps: SKIP_LOCAL=1 SKIP_BACKEND=1 SKIP_FRONTEND=1 SKIP_FINALIZE=1 SKIP_EXTENDED=1
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
DEPLOY="${ROOT}/deploy"

die() { echo "[pilot-final-deploy] ERROR: $*" >&2; exit 1; }

ssh_h() {
  ssh -o StrictHostKeyChecking=no "${USER}@$1" "${@:2}"
}

scp_smoke_tree() {
  local host=$1
  echo "==> Sync deploy/smoke → ${host}"
  ssh_h "$host" "mkdir -p /opt/aegis/deploy/smoke"
  scp -o StrictHostKeyChecking=no -r "${DEPLOY}/smoke/"* "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh_h "$host" "chmod +x /opt/aegis/deploy/smoke/*.sh"
}

deploy_backend_host() {
  local host=$1
  echo "==> Backend tarball + build on ${host}"
  local tar="/tmp/aegis-backend-final-$$.tar.gz"
  export COPYFILE_DISABLE=1
  tar --no-xattrs -czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto 2>/dev/null \
    || tar czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto
  scp -o StrictHostKeyChecking=no "$tar" "${USER}@${host}:/root/aegis-backend-final.tar.gz"
  rm -f "$tar"
  ssh_h "$host" bash -s <<'REMOTE'
set -euo pipefail
mkdir -p /opt/aegis
cfg_bak=""
[[ -f /opt/aegis/backend/config.yaml ]] && cfg_bak="$(mktemp)" && cp /opt/aegis/backend/config.yaml "$cfg_bak"
tar xzf /root/aegis-backend-final.tar.gz -C /opt/aegis
[[ -n "$cfg_bak" && -f "$cfg_bak" ]] && cp "$cfg_bak" /opt/aegis/backend/config.yaml && rm -f "$cfg_bak"
source /root/.cargo/env 2>/dev/null || true
cd /opt/aegis/backend
cargo build --release --bin agent-cli 2>&1 | tail -5
systemctl stop aegis-agent 2>/dev/null || true
cp target/release/agent-cli /opt/aegis/bin/agent-cli
chmod 755 /opt/aegis/bin/agent-cli
systemctl start aegis-agent
sleep 3
systemctl is-active aegis-agent
curl -sf http://127.0.0.1:8080/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d.get('status')=='ok', d"
echo "health OK on $(hostname)"
REMOTE
}

deploy_frontend_host() {
  local host=$1
  local out="${ROOT}/frontend/out"
  [[ -d "$out" ]] || die "missing frontend/out — run npm run build"
  echo "==> Frontend static → ${host}"
  tar czf - -C "$out" . | ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "rm -rf /root/frontend_staging /var/www/aegis/html/* && mkdir -p /root/frontend_staging /var/www/aegis/html && tar xzf - -C /root/frontend_staging && cp -a /root/frontend_staging/. /var/www/aegis/html/ && chmod -R 755 /var/www/aegis/html"
  ssh_h "$host" "grep -l SCOUT /var/www/aegis/html/_next/static/chunks/*.js 2>/dev/null | head -1 || echo WARN: SCOUT chunk not found"
}

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  AEGIS Pilot Final Deploy                                ║"
echo "╚══════════════════════════════════════════════════════════╝"

if [[ "${SKIP_LOCAL:-0}" != "1" ]]; then
  echo "==> [1/6] Local cargo build (no warnings)"
  (cd "${ROOT}/backend" && RUSTFLAGS="-D warnings" cargo build --release --bin agent-cli)
  echo "==> [2/6] Local frontend build"
  (cd "${ROOT}/frontend" && npm ci --no-audit 2>/dev/null || npm install --no-audit)
  (cd "${ROOT}/frontend" && npm run build)
fi

if [[ "${SKIP_BACKEND:-0}" != "1" ]]; then
  echo "==> [3/6] Sync smoke scripts + backend (primary)"
  scp_smoke_tree "$PRIMARY"
  deploy_backend_host "$PRIMARY"
  echo "==> [3/6] Sync smoke scripts + backend (secondary)"
  scp_smoke_tree "$SECONDARY"
  deploy_backend_host "$SECONDARY"
fi

if [[ "${SKIP_FRONTEND:-0}" != "1" ]]; then
  echo "==> [4/6] Frontend publish (both nodes)"
  deploy_frontend_host "$PRIMARY"
  deploy_frontend_host "$SECONDARY"
fi

if [[ "${SKIP_FINALIZE:-0}" != "1" ]]; then
  echo "==> [5/6] Monitoring + prod smoke gate"
  bash "${DEPLOY}/pilot-10-finalize.sh"
fi

if [[ "${SKIP_EXTENDED:-0}" != "1" ]]; then
  echo "==> [6/6] Extended integration smokes (from Mac)"
  export BASE_URL="https://aegis-security.ru"
  export SMOKE_API_KEY="${SMOKE_API_KEY:-$(ssh_h "$PRIMARY" "grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2- | tr -d '\"'")}"
  [[ -n "${SMOKE_API_KEY:-}" ]] || die "AEGIS_MONITOR_API_KEY missing on primary"
  export FEDERATION_SHARED_SECRET="${FEDERATION_SHARED_SECRET:-$(ssh_h "$PRIMARY" "grep '^FEDERATION_SHARED_SECRET=' /etc/aegis/agent.env | cut -d= -f2- | tr -d '\"'")}"
  [[ -n "${FEDERATION_SHARED_SECRET:-}" ]] || die "FEDERATION_SHARED_SECRET missing on primary agent.env"
  echo "==> phase4 feed mirrors (both nodes)"
  bash "${DEPLOY}/scout-sync-phase4-feeds.sh" "$PRIMARY"
  bash "${DEPLOY}/scout-sync-phase4-feeds.sh" "$SECONDARY"
  for s in \
    integration-scout-honest-10.sh \
    integration-scout-c1.sh \
    integration-scout-c2.sh \
    integration-federation-prod.sh; do
    echo "--- $s"
    bash "${DEPLOY}/smoke/$s"
  done
  if [[ "${SKIP_CHAOS:-0}" != "1" && -x "${DEPLOY}/federation-chaos/run-chaos-from-mac.sh" ]]; then
    echo "--- federation chaos (6/6)"
    CHAOS_CONFIRM=1 bash "${DEPLOY}/federation-chaos/run-chaos-from-mac.sh"
  fi
fi

echo ""
echo "=== pilot-final-deploy: ALL GATES PASSED ==="
echo "Primary:   https://aegis-security.ru/dashboard/"
echo "Secondary: https://node2.aegis-security.ru/dashboard/"
