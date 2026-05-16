#!/usr/bin/env bash
# PR6.1 — ReAct readiness endpoint vs JWT auth.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-test-key-enterprise}"
require_tools

body="$(curl -sf -X POST "$BASE/api/login" \
  -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
[[ -n "$TOKEN" ]] || die "login failed"

j="$(curl -sf -H "Authorization: Bearer $TOKEN" "$BASE/api/react/status")"
python3 -c "import sys,json; d=json.load(sys.stdin); assert isinstance(d, dict); print('react_status keys:', sorted(d.keys()))" <<<"$j"
echo "[integration-react-status] OK"
