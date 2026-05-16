#!/usr/bin/env bash
# Federation alert timer on secondary (local health + logs to journal).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

ssh_secondary() {
  if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${SECONDARY}" true 2>/dev/null; then
    ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" "$@"
  else
    [[ -n "${VPS_PASSWORD:-}" ]] || return 1
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" "$@"
  fi
}

scp_secondary() {
  if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${SECONDARY}" true 2>/dev/null; then
    scp -o StrictHostKeyChecking=no "$@"
  else
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e scp -o StrictHostKeyChecking=no "$@"
  fi
}

ssh_secondary "mkdir -p /opt/aegis/deploy/federation-alert"
# Share Telegram env from primary (token + chat_id only; chmod 600)
if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${PRIMARY_HOST:-178.236.16.101}" test -f /etc/aegis/federation-alert.env 2>/dev/null; then
  scp -o StrictHostKeyChecking=no "${USER}@${PRIMARY_HOST:-178.236.16.101}:/etc/aegis/federation-alert.env" /tmp/aegis-fed-alert.env.$$
  scp_secondary /tmp/aegis-fed-alert.env.$$ "${USER}@${SECONDARY}:/etc/aegis/federation-alert.env"
  ssh_secondary "chmod 600 /etc/aegis/federation-alert.env"
  rm -f /tmp/aegis-fed-alert.env.$$
fi
scp_secondary \
  "$ROOT/deploy/federation-alert/check-federation-alert-secondary.sh" \
  "$ROOT/deploy/federation-alert/aegis-federation-alert-secondary.service" \
  "$ROOT/deploy/federation-alert/aegis-federation-alert.timer" \
  "${USER}@${SECONDARY}:/opt/aegis/deploy/federation-alert/"

ssh_secondary bash -lc "
  install -m 755 /opt/aegis/deploy/federation-alert/check-federation-alert-secondary.sh /usr/local/bin/
  cp /opt/aegis/deploy/federation-alert/aegis-federation-alert.timer /etc/systemd/system/aegis-federation-alert-secondary.timer
  cp /opt/aegis/deploy/federation-alert/aegis-federation-alert-secondary.service /etc/systemd/system/
  systemctl daemon-reload
  systemctl enable --now aegis-federation-alert-secondary.timer
  systemctl is-active aegis-federation-alert-secondary.timer
"
echo "Secondary alert timer installed."
