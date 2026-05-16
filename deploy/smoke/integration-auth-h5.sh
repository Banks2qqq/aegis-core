#!/usr/bin/env bash
# H5 — hashed API keys: dev test-key blocked in prod; real key works.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY or AEGIS_MONITOR_API_KEY on host"

require_tools
DEV_MODE="$(grep '^AEGIS_DEV_MODE=' /etc/aegis/agent.env 2>/dev/null | cut -d= -f2- || echo 0)"

echo "[auth-h5] test-key-enterprise must fail when AEGIS_DEV_MODE=0"
if [[ "$DEV_MODE" == "0" || "$DEV_MODE" == "false" ]]; then
  code="$(curl -sS -o /tmp/aegis-auth-testkey.json -w "%{http_code}" \
    -X POST "$BASE/api/login" -H "Content-Type: application/json" \
    -d '{"api_key":"test-key-enterprise"}' || true)"
  [[ "$code" == "401" ]] || die "expected 401 for test-key in prod, got $code"
  echo "  401 as expected"
else
  echo "  skip (AEGIS_DEV_MODE=$DEV_MODE)"
fi

echo "[auth-h5] wrong key → 401"
code_bad="$(curl -sS -o /dev/null -w "%{http_code}" \
  -X POST "$BASE/api/login" -H "Content-Type: application/json" \
  -d '{"api_key":"totally-invalid-key-000"}' || true)"
[[ "$code_bad" == "401" ]] || die "invalid key expected 401 got $code_bad"

echo "[auth-h5] monitor key → 200"
body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$API_KEY\"}")"
echo "$body" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d.get('access_token'), d"
echo "  login OK tier=$(echo "$body" | python3 -c "import json,sys;print(json.load(sys.stdin).get('tier','?'))")"

echo "[integration-auth-h5] OK"
