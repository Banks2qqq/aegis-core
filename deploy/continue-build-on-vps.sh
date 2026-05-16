#!/usr/bin/env bash
# Mac only: fix protoc + finish backend build after a failed bootstrap.
set -euo pipefail
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Run on Mac. On server type: exit" >&2
  exit 1
fi
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export VPS_HOST="${VPS_HOST:-178.236.16.101}"
export VPS_USER="${VPS_USER:-root}"
# shellcheck source=ssh-mux.sh
source "${ROOT}/deploy/ssh-mux.sh"
trap ssh_mux_close EXIT
ssh_mux_open
echo "==> Upload proto (required for build)"
scp_dir_cmd "${ROOT}/proto" "${VPS_USER}@${VPS_HOST}:/opt/aegis/"
ssh_cmd "apt-get update -qq && apt-get install -y -qq protobuf-compiler"
ssh_cmd 'bash -lc "source /root/.cargo/env && cd /opt/aegis/backend && cargo build --release --bin agent-cli"'
ssh_cmd "systemctl stop aegis-agent; cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli && chmod 755 /opt/aegis/bin/agent-cli"
ssh_cmd "systemctl start aegis-agent && sleep 2 && systemctl is-active aegis-agent"
if [[ -d "${ROOT}/frontend/out" ]]; then
  export SSH_CONTROL_PATH
  "${ROOT}/deploy_to_vps.sh"
fi
echo "Backend OK. Run SSL if needed: ssh root@${VPS_HOST} certbot --nginx -d aegis-security.ru -d www.aegis-security.ru"
