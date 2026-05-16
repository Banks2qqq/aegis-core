#!/usr/bin/env bash
# Two-node federation smoke on production (JWT + federation token).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

PRIMARY="${PRIMARY_URL:-https://aegis-security.ru}"
SECONDARY="${SECONDARY_URL:-https://node2.aegis-security.ru}"
API_KEY="${SMOKE_API_KEY:-test-key-enterprise}"
FED_SECRET="${FEDERATION_SHARED_SECRET:?Set FEDERATION_SHARED_SECRET from apply-federation-cluster.sh output}"

require_tools
HDR=( -H "X-AEGIS-Federation-Token: $FED_SECRET" )

echo "[fed-prod] Primary health"
curl -k -sfS "$PRIMARY/health" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"

echo "[fed-prod] Secondary health"
curl -sfS "$SECONDARY/health" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"

TOK=$(curl -sfS -X POST "$PRIMARY/api/login" -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$API_KEY\"}" | python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])")

echo "[fed-prod] Federation health report"
curl -k -sfS -H "Authorization: Bearer $TOK" "$PRIMARY/api/federation/health" | python3 -m json.tool | head -40

echo "[fed-prod] Sync all from primary"
curl -k -sfS -H "Authorization: Bearer $TOK" -H "Content-Type: application/json" \
  -X POST "$PRIMARY/api/federation/sync" -d '{"sync_all":true}' | python3 -m json.tool

echo "[fed-prod] Port 8443 mTLS checks"
bash "$ROOT/deploy/smoke/check-federation-mtls-port.sh"

echo "[fed-prod] OK"
