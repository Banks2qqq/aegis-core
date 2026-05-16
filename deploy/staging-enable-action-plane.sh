#!/usr/bin/env bash
# Staging = secondary node: enable real heal apply + contain enforce.
# Production primary stays dry-run / policy-only.
set -euo pipefail
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

# shellcheck source=ssh-mux.sh
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "${ROOT}/deploy/ssh-mux.sh"

upsert_env() {
  local host=$1 key=$2 val=$3
  export VPS_HOST="$host"
  ssh_cmd "touch /etc/aegis/agent.env && chmod 600 /etc/aegis/agent.env
    if grep -q '^${key}=' /etc/aegis/agent.env 2>/dev/null; then
      sed -i 's|^${key}=.*|${key}=${val}|' /etc/aegis/agent.env
    else
      echo '${key}=${val}' >> /etc/aegis/agent.env
    fi"
}

apply_host() {
  local host=$1 heal=$2 contain=$3 label=$4
  export VPS_HOST="$host"
  SSH_CTL="/tmp/aegis-staging-${USER}-${host}.sock"
  export SSH_CONTROL_PATH="$SSH_CTL"
  echo "==> [$label @ $host] AEGIS_HEAL_APPLY=$heal AEGIS_CONTAIN_ENFORCE=$contain"
  ssh_mux_open
  upsert_env "$host" AEGIS_HEAL_APPLY "$heal"
  upsert_env "$host" AEGIS_CONTAIN_ENFORCE "$contain"
  ssh_cmd "systemctl restart aegis-agent && sleep 3 && systemctl is-active aegis-agent"
  ssh_mux_close
}

apply_host "$PRIMARY" "0" "0" "production"
apply_host "$SECONDARY" "1" "1" "staging"
echo "=== staging action plane enabled (secondary only) ==="
