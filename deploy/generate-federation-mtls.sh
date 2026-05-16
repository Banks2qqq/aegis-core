#!/usr/bin/env bash
# PR — Generate federation CA + client certs for primary and secondary nodes.
# Output: deploy/federation-certs/ (do not commit)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${ROOT}/deploy/federation-certs"
DAYS="${MTLS_DAYS:-825}"

mkdir -p "$OUT"
cd "$OUT"

if [[ -f ca.pem && -f primary.client.pem && -f secondary.client.pem && "${MTLS_REGENERATE:-0}" != "1" ]]; then
  echo "[mtls] Certs already exist in $OUT — set MTLS_REGENERATE=1 to recreate"
  exit 0
fi

echo "[mtls] Generating CA..."
openssl genrsa -out ca.key 4096 2>/dev/null
openssl req -new -x509 -days "$DAYS" -key ca.key -out ca.pem \
  -subj "/CN=AEGIS Federation CA/O=AEGIS Pilot"

gen_client() {
  local name=$1
  echo "[mtls] Client cert: $name"
  openssl genrsa -out "${name}.client.key" 2048 2>/dev/null
  openssl req -new -key "${name}.client.key" -out "${name}.client.csr" \
    -subj "/CN=aegis-federation-${name}"
  openssl x509 -req -days "$DAYS" -in "${name}.client.csr" \
    -CA ca.pem -CAkey ca.key -CAcreateserial \
    -out "${name}.client.pem" 2>/dev/null
  chmod 600 "${name}.client.key" ca.key
  rm -f "${name}.client.csr"
}

gen_client primary
gen_client secondary

# Combined PEM for reqwest::Identity
cat secondary.client.pem secondary.client.key > secondary.client-bundle.pem
cat primary.client.pem primary.client.key > primary.client-bundle.pem

echo "[mtls] Done → $OUT"
ls -la "$OUT"/*.pem "$OUT"/*.key 2>/dev/null | awk '{print $NF}'
