#!/usr/bin/env bash
# PR4 deploy: backend with patch_applier + contain_enforcer + llm_status
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VPS="${VPS:-root@178.236.16.101}"
BACKEND_REMOTE="/opt/aegis/backend"
BIN_REMOTE="/opt/aegis/bin/agent-cli"

echo "[PR4] Building backend..."
(cd "$ROOT/backend" && cargo build --release --bin agent-cli)

echo "[PR4] Upload + restart agent on $VPS"
scp "$ROOT/backend/target/release/agent-cli" "$VPS:$BIN_REMOTE.new"
scp "$ROOT/backend/src/agent/patch_applier.rs" "$VPS:$BACKEND_REMOTE/src/agent/"
scp "$ROOT/backend/src/agent/contain_enforcer.rs" "$VPS:$BACKEND_REMOTE/src/agent/"
scp "$ROOT/backend/src/agent/llm_status.rs" "$VPS:$BACKEND_REMOTE/src/agent/"
ssh "$VPS" "bash -lc 'systemctl stop aegis-agent; mv -f $BIN_REMOTE.new $BIN_REMOTE; systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent'"

echo "[PR4] Done. Set AEGIS_HEAL_APPLY / AEGIS_CONTAIN_ENFORCE in /etc/aegis/agent.env"
