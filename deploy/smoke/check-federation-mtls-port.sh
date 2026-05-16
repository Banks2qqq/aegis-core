#!/usr/bin/env bash
# Monitor federation mTLS port 8443 (TCP + TLS handshake with client cert).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

PRIMARY_HOST="${PRIMARY_HOST:-aegis-security.ru}"
NODE2_HOST="${NODE2_HOST:-node2.aegis-security.ru}"
CERTS="${ROOT}/deploy/federation-certs"
FED_SECRET="${FEDERATION_SHARED_SECRET:-}"

require_tools

check_port() {
  local host=$1
  echo "[mtls-port] TCP ${host}:8443"
  if command -v nc >/dev/null 2>&1; then
    nc -z -w 5 "$host" 8443
  else
    timeout 5 bash -c "echo >/dev/tcp/${host}/8443"
  fi
}

check_mtls_handshake() {
  local host=$1
  local label=$2
  echo "[mtls-port] Handshake ${label} ${host}:8443 (with client cert)"
  [[ -f "$CERTS/primary.client.pem" ]] || {
    echo "Missing $CERTS — run ./deploy/generate-federation-mtls.sh" >&2
    exit 1
  }
  local out
  out=$(openssl s_client -connect "${host}:8443" -servername "$host" \
    -cert "$CERTS/primary.client.pem" -key "$CERTS/primary.client.key" \
    </dev/null 2>&1 | grep -E "Verify return code|CN = aegis-federation" | head -3 || true)
  echo "$out"
  echo "$out" | grep -q "Verify return code: 0" || {
    echo "TLS verify failed for ${host}:8443" >&2
    exit 1
  }
}

check_no_cert_rejected() {
  local host=$1
  echo "[mtls-port] ${host}:8443 without client cert should fail TLS"
  if openssl s_client -connect "${host}:8443" -servername "$host" </dev/null 2>&1 | grep -qi "alert\|handshake failure\|certificate required"; then
    echo "  OK (rejected)"
  else
    local code
    code=$(curl -sk -o /dev/null -w "%{http_code}" "https://${host}:8443/federation/merkle" || echo "000")
    [[ "$code" == "400" || "$code" == "403" || "$code" == "495" ]] || {
      echo "Expected rejection, got HTTP ${code}" >&2
      exit 1
    }
    echo "  OK (HTTP ${code})"
  fi
}

check_federation_api() {
  local host=$1
  [[ -n "$FED_SECRET" ]] || return 0
  echo "[mtls-port] GET https://${host}:8443/federation/merkle (cert + token)"
  curl -sfS \
    --cert "$CERTS/primary.client.pem" \
    --key "$CERTS/primary.client.key" \
    -H "X-AEGIS-Federation-Token: $FED_SECRET" \
    "https://${host}:8443/federation/merkle" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d.get('merkle_root') or d.get('root')"
}

check_port "$NODE2_HOST"
check_port "$PRIMARY_HOST"
check_mtls_handshake "$NODE2_HOST" "node2"
check_no_cert_rejected "$NODE2_HOST"
check_federation_api "$NODE2_HOST"
echo "[mtls-port] All checks passed"
