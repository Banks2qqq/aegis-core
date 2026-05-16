#!/usr/bin/env bash
# H3 — deploy HITL heal queue (secondary first, then primary). Run from Mac.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

deploy_backend() {
  local host=$1
  echo "==> Backend deploy → ${host}"
  local tar="/tmp/aegis-h3-$$.tar.gz"
  export COPYFILE_DISABLE=1
  tar --no-xattrs -czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto 2>/dev/null \
    || tar czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto
  scp -o StrictHostKeyChecking=no "$tar" "${USER}@${host}:/root/aegis-h3.tar.gz"
  rm -f "$tar"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<'REMOTE'
set -euo pipefail
cfg_bak=""
[[ -f /opt/aegis/backend/config.yaml ]] && cfg_bak="$(mktemp)" && cp /opt/aegis/backend/config.yaml "$cfg_bak"
mkdir -p /opt/aegis
tar xzf /root/aegis-h3.tar.gz -C /opt/aegis
[[ -n "$cfg_bak" && -f "$cfg_bak" ]] && cp "$cfg_bak" /opt/aegis/backend/config.yaml && rm -f "$cfg_bak"
source /root/.cargo/env 2>/dev/null || true
cd /opt/aegis/backend && cargo build --release --bin agent-cli
systemctl stop aegis-agent
cp target/release/agent-cli /opt/aegis/bin/agent-cli
chmod 755 /opt/aegis/bin/agent-cli
systemctl start aegis-agent
sleep 3
systemctl is-active aegis-agent
curl -sf http://127.0.0.1:8080/health | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='ok'"
REMOTE
}

sync_smoke() {
  local host=$1
  scp -o StrictHostKeyChecking=no \
    "${ROOT}/deploy/smoke/lib.sh" \
    "${ROOT}/deploy/smoke/integration-heal-hitl.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "chmod +x /opt/aegis/deploy/smoke/*.sh"
}

run_smoke() {
  local host=$1
  echo "==> smoke integration-heal-hitl on ${host}"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "BASE_URL=http://127.0.0.1:8080 /opt/aegis/deploy/smoke/integration-heal-hitl.sh"
}

deploy_backend "$SECONDARY"
sync_smoke "$SECONDARY"
run_smoke "$SECONDARY"

deploy_backend "$PRIMARY"
sync_smoke "$PRIMARY"
run_smoke "$PRIMARY"

echo "==> H3 HITL heal deploy complete (secondary + primary)"
