#!/usr/bin/env bash
# Production refine: real heal/deception APIs, HITL UI, landing metrics. Run from Mac.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

deploy_backend() {
  local host=$1
  echo "==> Backend → ${host}"
  local tar="/tmp/aegis-prod-$$.tar.gz"
  export COPYFILE_DISABLE=1
  tar --no-xattrs -czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto 2>/dev/null \
    || tar czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto
  scp -o StrictHostKeyChecking=no "$tar" "${USER}@${host}:/root/aegis-prod.tar.gz"
  rm -f "$tar"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<'REMOTE'
set -euo pipefail
cfg_bak=""
[[ -f /opt/aegis/backend/config.yaml ]] && cfg_bak="$(mktemp)" && cp /opt/aegis/backend/config.yaml "$cfg_bak"
mkdir -p /opt/aegis
tar xzf /root/aegis-prod.tar.gz -C /opt/aegis
[[ -n "$cfg_bak" && -f "$cfg_bak" ]] && cp "$cfg_bak" /opt/aegis/backend/config.yaml && rm -f "$cfg_bak"
source /root/.cargo/env 2>/dev/null || true
cd /opt/aegis/backend && cargo build --release --bin agent-cli
systemctl stop aegis-agent
cp target/release/agent-cli /opt/aegis/bin/agent-cli
systemctl start aegis-agent
sleep 3
systemctl is-active aegis-agent
REMOTE
}

sync_smokes() {
  local host=$1
  scp -o StrictHostKeyChecking=no \
    "${ROOT}/deploy/smoke/lib.sh" \
    "${ROOT}/deploy/smoke/integration-heal-hitl.sh" \
    "${ROOT}/deploy/smoke/integration-deception-h2.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" "chmod +x /opt/aegis/deploy/smoke/*.sh"
}

run_smokes() {
  local host=$1
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "BASE_URL=http://127.0.0.1:8080 /opt/aegis/deploy/smoke/integration-heal-hitl.sh"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "BASE_URL=http://127.0.0.1:8080 /opt/aegis/deploy/smoke/integration-deception-h2.sh"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "curl -sf http://127.0.0.1:8080/api/status/public | python3 -c \"import sys,json; d=json.load(sys.stdin); assert d.get('status')=='ok'\""
}

publish_frontend() {
  local host=$1
  tar czf - -C "${ROOT}/frontend/out" . | ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "rm -rf /root/frontend_staging /var/www/aegis/html/* && mkdir -p /root/frontend_staging /var/www/aegis/html && tar xzf - -C /root/frontend_staging && cp -a /root/frontend_staging/. /var/www/aegis/html/ && chmod -R 755 /var/www/aegis/html"
}

for host in "$SECONDARY" "$PRIMARY"; do
  deploy_backend "$host"
  sync_smokes "$host"
  run_smokes "$host"
  publish_frontend "$host"
done

echo "==> production-refine-deploy complete"
