#!/usr/bin/env bash
# PR6.1 — Agents / ops-plane sanity (includes healer/scout/registry).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"
BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-test-key-enterprise}"
require_tools
body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
json="$(curl -sf -H "Authorization: Bearer $TOKEN" "$BASE/api/agents")"
python3 -c "import sys,json; a=json.load(sys.stdin); assert isinstance(a,list); assert len(a)>=1" <<<"$json"
echo "[integration-healing-registry] agents payload OK"
