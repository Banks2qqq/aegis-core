#!/usr/bin/env bash
# PR5 deploy: federation auth, merkle sync, optional background sync
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VPS="${VPS:-root@178.236.16.101}"
BACKEND_REMOTE="/opt/aegis/backend"
BIN_REMOTE="/opt/aegis/bin/agent-cli"
ENV_REMOTE="/etc/aegis/agent.env"

echo "[PR5] Building backend..."
(cd "$ROOT/backend" && cargo build --release --bin agent-cli)

echo "[PR5] Upload sources + binary to $VPS"
scp "$ROOT/backend/target/release/agent-cli" "$VPS:$BIN_REMOTE.new"
for f in federation.rs federation_auth.rs federation_client.rs config.rs server.rs main.rs; do
  scp "$ROOT/backend/src/agent/$f" "$VPS:$BACKEND_REMOTE/src/agent/"
done
scp "$ROOT/deploy/pr5-federation-smoke.sh" "$ROOT/deploy/config.federation-peer.example.yaml" "$VPS:/opt/aegis/deploy/" 2>/dev/null || true
scp "$ROOT/deploy/config.production.yaml" "$VPS:$BACKEND_REMOTE/config.yaml"
scp "$ROOT/deploy/config.federation-peer.example.yaml" "$VPS:$BACKEND_REMOTE/"

ssh "$VPS" "bash -lc 'systemctl stop aegis-agent; mv -f $BIN_REMOTE.new $BIN_REMOTE; systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent'"

echo "[PR5] Done. Set FEDERATION_SHARED_SECRET in $ENV_REMOTE on every federated node."
