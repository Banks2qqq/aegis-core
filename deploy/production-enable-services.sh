#!/usr/bin/env bash
# Enable autostart for AEGIS stack on primary + secondary (run after any deploy).
set -euo pipefail
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

enable_on() {
  local host=$1
  echo "==> ${host}"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -lc "
    systemctl enable aegis-agent
    systemctl enable nginx 2>/dev/null || true
    systemctl enable aegis-federation-alert.timer 2>/dev/null || true
    systemctl is-enabled aegis-agent
    systemctl is-active aegis-agent
  "
}

ssh_secondary() {
  if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${SECONDARY}" true 2>/dev/null; then
    enable_on "$SECONDARY"
  elif [[ -n "${VPS_PASSWORD:-}" ]]; then
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" bash -lc "
      systemctl enable aegis-agent
      systemctl enable nginx 2>/dev/null || true
      systemctl is-enabled aegis-agent
      systemctl is-active aegis-agent
    "
  else
    echo "Skip secondary — set VPS_PASSWORD or SSH key" >&2
    return 1
  fi
}

enable_on "$PRIMARY"
ssh_secondary || true
echo "Done."
