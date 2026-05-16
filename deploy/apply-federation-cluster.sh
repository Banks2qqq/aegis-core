#!/usr/bin/env bash
# Set shared federation secret on both nodes, update configs, restart agents.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

SECRET="${FEDERATION_SHARED_SECRET:-}"
if [[ -z "$SECRET" ]]; then
  SECRET="$(openssl rand -hex 32)"
  echo "[federation] Generated new FEDERATION_SHARED_SECRET (save it!):"
  echo "$SECRET"
fi

apply_env() {
  local host=$1
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s -- "$SECRET" <<'REMOTE'
set -euo pipefail
SECRET="$1"
ENV=/etc/aegis/agent.env
touch "$ENV"
chmod 600 "$ENV"
if grep -q '^FEDERATION_SHARED_SECRET=' "$ENV"; then
  sed -i "s|^FEDERATION_SHARED_SECRET=.*|FEDERATION_SHARED_SECRET=${SECRET}|" "$ENV"
else
  echo "FEDERATION_SHARED_SECRET=${SECRET}" >>"$ENV"
fi
grep -q '^AEGIS_FEDERATION_CA_CERT=' "$ENV" || echo 'AEGIS_FEDERATION_CA_CERT=/etc/aegis/federation/ca.pem' >>"$ENV"
sed -i '/^AEGIS_FEDERATION_INSECURE_TLS=/d' "$ENV"
REMOTE
}

echo "==> agent.env on both nodes"
apply_env "$PRIMARY"
apply_env "$SECONDARY"

echo "==> config.yaml"
scp -o StrictHostKeyChecking=no "${ROOT}/deploy/config.primary-with-peer.yaml" \
  "${USER}@${PRIMARY}:/opt/aegis/backend/config.yaml"
scp -o StrictHostKeyChecking=no "${ROOT}/deploy/config.secondary.production.yaml" \
  "${USER}@${SECONDARY}:/opt/aegis/backend/config.yaml"

echo "==> patch federation_client.rs on both nodes"
for host in "$PRIMARY" "$SECONDARY"; do
  scp -o StrictHostKeyChecking=no "${ROOT}/backend/src/agent/federation_client.rs" \
    "${USER}@${host}:/opt/aegis/backend/src/agent/federation_client.rs"
done

echo "==> rebuild agent + restart"
for host in "$PRIMARY" "$SECONDARY"; do
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    'bash -lc "source /root/.cargo/env 2>/dev/null; cd /opt/aegis/backend && cargo build --release --bin agent-cli"'
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "systemctl stop aegis-agent; cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli; systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent"
done

echo "[federation] Cluster config applied"
echo "Save FEDERATION_SHARED_SECRET for smoke:"
echo "  export FEDERATION_SHARED_SECRET='${SECRET}'"
echo "  ./deploy/smoke/integration-federation-prod.sh"
