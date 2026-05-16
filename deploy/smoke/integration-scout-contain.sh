#!/usr/bin/env bash
# PR6.1 — Scouts optional (LLM/external); contain always exercised with synthetic cluster_id.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || API_KEY="test-key-enterprise"
SKIP_SCOUT="${SKIP_SCOUT:-1}"
SCOUT_TIMEOUT="${SCOUT_TIMEOUT:-120}"

require_tools

body="$(curl -sf -X POST "$BASE/api/login" \
  -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
[[ -n "$TOKEN" ]] || die "login failed"

auth_hdr=( -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" )

if [[ "$SKIP_SCOUT" != "1" ]]; then
  SCOUT_TMP="$(mktemp)"
  echo "[scout-contain] POST /api/scout (timeout ${SCOUT_TIMEOUT}s) ..."
  code="$(curl -sS -m "$SCOUT_TIMEOUT" -o "$SCOUT_TMP" -w "%{http_code}" \
    "${auth_hdr[@]}" -X POST "$BASE/api/scout" -d '{}' || true)"
  rm -f "$SCOUT_TMP"
  [[ "$code" == "200" ]] || die "/api/scout expected 200 got $code"
fi

CLUSTER_ID="smoke-cluster-$(date +%s)"
resp="$(curl -sS "${auth_hdr[@]}" -X POST "$BASE/api/contain" \
  -d "{\"cluster_id\":\"$CLUSTER_ID\"}")"
status="$(python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" <<<"$resp")"
host_enforced="$(python3 -c "import sys,json; print(json.load(sys.stdin).get('host_enforced',False))" <<<"$resp")"
enforcement_mode="$(python3 -c "import sys,json; print(json.load(sys.stdin).get('enforcement_mode',''))" <<<"$resp")"
[[ "$status" == "contained" ]] || die "unexpected contain status=$status body=$resp"
EXPECT_ENFORCE="${EXPECT_CONTAIN_ENFORCE:-0}"
if [[ "$EXPECT_ENFORCE" == "1" ]]; then
  [[ "$host_enforced" == "True" || "$host_enforced" == "true" ]] || \
    die "expected host_enforced=true got $host_enforced mode=$enforcement_mode"
  echo "[scout-contain] host enforcement OK (mode=$enforcement_mode)"
fi
echo "[scout-contain] contain path OK (cluster=$CLUSTER_ID status=$status)"

echo "[integration-scout-contain] OK"
