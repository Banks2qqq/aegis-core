#!/usr/bin/env bash
# Install your SSH public key on secondary (one-time); requires VPS_PASSWORD or existing access.
set -euo pipefail
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
PUB="${1:-${HOME}/.ssh/id_rsa.pub}"
[[ -f "$PUB" ]] || PUB="${HOME}/.ssh/id_ed25519.pub"
[[ -f "$PUB" ]] || { echo "No SSH public key found" >&2; exit 1; }

if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${SECONDARY}" true 2>/dev/null; then
  echo "SSH key already works for ${SECONDARY}"
  exit 0
fi

[[ -n "${VPS_PASSWORD:-}" ]] || { echo "Set VPS_PASSWORD" >&2; exit 1; }
export SSHPASS="$VPS_PASSWORD"
sshpass -e ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" \
  "mkdir -p ~/.ssh && chmod 700 ~/.ssh && cat >> ~/.ssh/authorized_keys" < "$PUB"
sshpass -e ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" \
  "chmod 600 ~/.ssh/authorized_keys && sort -u ~/.ssh/authorized_keys -o ~/.ssh/authorized_keys"

ssh -o BatchMode=yes "${USER}@${SECONDARY}" echo "Key installed OK"
