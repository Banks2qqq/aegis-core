#!/usr/bin/env bash
# H1 — deploy real Docker sandbox (secondary first, then primary). Run from Mac.
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
  "AEGIS_SANDBOX_RUNTIME=docker" \
  "AEGIS_SANDBOX_IMAGE=alpine:3.20" \
  "AEGIS_SANDBOX_TIMEOUT_SECS=120"; do
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
echo "docker OK on $(hostname)"
REMOTE
}

deploy_backend() {
  local host=$1
  echo "==> Backend deploy → ${host}"
  local tar="/tmp/aegis-h1-$$.tar.gz"
  export COPYFILE_DISABLE=1
  tar --no-xattrs -czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto 2>/dev/null \
    || tar czf "$tar" -C "$ROOT" --exclude='backend/target' backend proto
  scp -o StrictHostKeyChecking=no "$tar" "${USER}@${host}:/root/aegis-h1.tar.gz"
  rm -f "$tar"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<'REMOTE'
set -euo pipefail
cfg_bak=""
[[ -f /opt/aegis/backend/config.yaml ]] && cfg_bak="$(mktemp)" && cp /opt/aegis/backend/config.yaml "$cfg_bak"
mkdir -p /opt/aegis
tar xzf /root/aegis-h1.tar.gz -C /opt/aegis
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
    "${ROOT}/deploy/smoke/integration-heal-sandbox-real.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "chmod +x /opt/aegis/deploy/smoke/*.sh"
}

run_smoke() {
  local host=$1
  local base=$2
  echo "==> smoke integration-heal-sandbox-real on ${host}"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<REMOTE
set -euo pipefail
source /etc/aegis/agent.env
export BASE_URL="${base}"
export SMOKE_API_KEY="\$AEGIS_MONITOR_API_KEY"
bash /opt/aegis/deploy/smoke/integration-heal-sandbox-real.sh
REMOTE
}

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  H1 — Real Docker sandbox deploy                         ║"
echo "╚══════════════════════════════════════════════════════════╝"

echo "==> [1/4] Env + docker check (secondary, primary)"
patch_env "$SECONDARY"
patch_env "$PRIMARY"

echo "==> [2/4] Deploy secondary (staging)"
deploy_backend "$SECONDARY"
sync_smoke "$SECONDARY"
run_smoke "$SECONDARY" "https://node2.aegis-security.ru"

echo "==> [3/4] Deploy primary"
deploy_backend "$PRIMARY"
sync_smoke "$PRIMARY"
run_smoke "$PRIMARY" "https://aegis-security.ru"

echo ""
echo "=== H1 sandbox deploy: PASS ==="
