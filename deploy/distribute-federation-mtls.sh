#!/usr/bin/env bash
# Copy federation mTLS material to primary + secondary VPS.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CERTS="${ROOT}/deploy/federation-certs"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

[[ -f "$CERTS/ca.pem" ]] || { echo "Run ./deploy/generate-federation-mtls.sh first"; exit 1; }

remote() {
  local host=$1
  shift
  if [[ "$host" == "$SECONDARY" ]] && ! ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${host}" true 2>/dev/null; then
    [[ -n "${VPS_PASSWORD:-}" ]] || { echo "Set VPS_PASSWORD for ${host}" >&2; exit 1; }
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e ssh -o StrictHostKeyChecking=no "${USER}@${host}" "$@"
  else
    ssh -o StrictHostKeyChecking=no "${USER}@${host}" "$@"
  fi
}

copy() {
  local host=$1 src=$2 dest=$3
  if [[ "$host" == "$SECONDARY" ]] && ! ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${host}" true 2>/dev/null; then
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e scp -o StrictHostKeyChecking=no "$src" "${USER}@${host}:${dest}"
  else
    scp -o StrictHostKeyChecking=no "$src" "${USER}@${host}:${dest}"
  fi
}

for host in "$PRIMARY" "$SECONDARY"; do
  echo "==> $host"
  remote "$host" "mkdir -p /etc/aegis/federation && chmod 700 /etc/aegis/federation"
  copy "$host" "$CERTS/ca.pem" /etc/aegis/federation/ca.pem
done

copy "$PRIMARY" "$CERTS/primary.client.pem" /etc/aegis/federation/primary.client.pem
copy "$PRIMARY" "$CERTS/primary.client.key" /etc/aegis/federation/primary.client.key
copy "$SECONDARY" "$CERTS/secondary.client.pem" /etc/aegis/federation/secondary.client.pem
copy "$SECONDARY" "$CERTS/secondary.client.key" /etc/aegis/federation/secondary.client.key

remote "$PRIMARY" "chmod 600 /etc/aegis/federation/*.key"
remote "$SECONDARY" "chmod 600 /etc/aegis/federation/*.key"

# reqwest native-tls expects PKCS#8 private keys
for host in "$PRIMARY" "$SECONDARY"; do
  remote "$host" 'for f in /etc/aegis/federation/*.client.key; do
    openssl pkcs8 -topk8 -nocrypt -in "$f" -out "${f}.pkcs8" && mv "${f}.pkcs8" "$f" && chmod 600 "$f"
  done'
done

echo "[mtls] Distributed to ${PRIMARY} and ${SECONDARY}"
