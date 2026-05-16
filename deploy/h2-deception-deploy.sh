#!/usr/bin/env bash
# H2 — deploy Docker deception listeners (secondary first, then primary). Run from Mac.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

patch_env() {
  local host=$1
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<'REMOTE'
set -euo pipefail
ENV=/etc/aegis/agent.env
touch "$ENV"
chmod 600 "$ENV"
for kv in \
  "AEGIS_DECEPTION_RUNTIME=docker" \
  "AEGIS_DECEPTION_IMAGE=nginx:alpine" \
  "AEGIS_DECEPTION_SMOKE_PORT=19080"; do
  key="${kv%%=*}"
  if grep -q "^${key}=" "$ENV"; then
    sed -i "s|^${key}=.*|${kv}|" "$ENV"
  else
    echo "$kv" >>"$ENV"
  fi
done
if ! command -v docker >/dev/null 2>&1; then
  echo "Installing docker.io on $(hostname)..."
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y -qq docker.io
  systemctl enable --now docker
fi
docker info >/dev/null 2>&1 || { echo "ERROR: docker not available on $(hostname)" >&2; exit 1; }
docker pull nginx:alpine >/dev/null 2>&1 || true
echo "docker OK on $(hostname)"
REMOTE
}

deploy_backend() {
  local host=$1
  echo "==> Backend deploy → ${host}"
  local tar="/tmp/aegis-h2-$$.tar.gz"
  export COPYFILE_DISABLE=1
  tar --no-xattrs -czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto 2>/dev/null \
    || tar czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto
  scp -o StrictHostKeyChecking=no "$tar" "${USER}@${host}:/root/aegis-h2.tar.gz"
  rm -f "$tar"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<'REMOTE'
set -euo pipefail
cfg_bak=""
[[ -f /opt/aegis/backend/config.yaml ]] && cfg_bak="$(mktemp)" && cp /opt/aegis/backend/config.yaml "$cfg_bak"
mkdir -p /opt/aegis
tar xzf /root/aegis-h2.tar.gz -C /opt/aegis
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
    "${ROOT}/deploy/smoke/integration-deception-h2.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "chmod +x /opt/aegis/deploy/smoke/*.sh"
}

run_smoke() {
  local host=$1
  echo "==> smoke integration-deception-h2 on ${host}"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "BASE_URL=http://127.0.0.1:8080 /opt/aegis/deploy/smoke/integration-deception-h2.sh"
}

patch_env "$SECONDARY"
deploy_backend "$SECONDARY"
sync_smoke "$SECONDARY"
run_smoke "$SECONDARY"

patch_env "$PRIMARY"
deploy_backend "$PRIMARY"
sync_smoke "$PRIMARY"
run_smoke "$PRIMARY"

echo "==> H2 deception deploy complete (secondary + primary)"
