#!/usr/bin/env bash
# H5 + H6 — hashed API keys + nginx /metrics. Run from Mac.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

deploy_backend() {
  local host=$1
  echo "==> Backend → ${host}"
  local tar="/tmp/aegis-h56-$$.tar.gz"
  export COPYFILE_DISABLE=1
  tar --no-xattrs -czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto 2>/dev/null \
    || tar czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto
  scp -o StrictHostKeyChecking=no "$tar" "${USER}@${host}:/root/aegis-h56.tar.gz"
  rm -f "$tar"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<'REMOTE'
set -euo pipefail
cfg_bak=""
[[ -f /opt/aegis/backend/config.yaml ]] && cfg_bak="$(mktemp)" && cp /opt/aegis/backend/config.yaml "$cfg_bak"
mkdir -p /opt/aegis
tar xzf /root/aegis-h56.tar.gz -C /opt/aegis
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

apply_nginx() {
  local host=$1
  local conf=$2
  local name=$3
  echo "==> nginx ${name} → ${host}"
  scp -o StrictHostKeyChecking=no "${ROOT}/deploy/${conf}" "${USER}@${host}:/etc/nginx/sites-available/${name}"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" "nginx -t && systemctl reload nginx"
}

sync_smokes() {
  local host=$1
  scp -o StrictHostKeyChecking=no \
    "${ROOT}/deploy/smoke/lib.sh" \
    "${ROOT}/deploy/smoke/integration-auth-h5.sh" \
    "${ROOT}/deploy/smoke/integration-nginx-metrics-h6.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" "chmod +x /opt/aegis/deploy/smoke/*.sh"
}

run_smokes_local() {
  local host=$1
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "BASE_URL=http://127.0.0.1:8080 /opt/aegis/deploy/smoke/integration-auth-h5.sh"
}

run_smokes_public() {
  local url=$1
  ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
    "PUBLIC_URL=${url} /opt/aegis/deploy/smoke/integration-nginx-metrics-h6.sh" || true
}

for host in "$SECONDARY" "$PRIMARY"; do
  deploy_backend "$host"
  sync_smokes "$host"
  run_smokes_local "$host"
done

apply_nginx "$PRIMARY" "nginx-aegis-full.conf" "aegis"
apply_nginx "$SECONDARY" "nginx-aegis-node2-full.conf" "aegis-node2"

# H6 public smoke from primary (has monitor key in env)
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
  "PUBLIC_URL=https://aegis-security.ru /opt/aegis/deploy/smoke/integration-nginx-metrics-h6.sh"

echo "==> H5+H6 deploy complete"
