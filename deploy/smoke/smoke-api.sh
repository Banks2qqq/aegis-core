#!/usr/bin/env bash
# PR6.1 — Fast API smoke: health, login, protected reads.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-test-key-enterprise}"

require_tools

echo "[smoke-api] BASE=$BASE"

code="$(http_code -sf "$BASE/health")" || true
[[ "$code" == "200" ]] || die "GET /health expected 200 got $code"

body="$(curl -sf -X POST "$BASE/api/login" \
  -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
[[ -n "${TOKEN:-}" ]] || die "no access_token from login"

auth=( -H "Authorization: Bearer $TOKEN" )

for path in /api/status /api/knowledge /api/threats /api/agents /api/react/status; do
  c="$(curl -sS -o /dev/null -w "%{http_code}" "${auth[@]}" "$BASE$path")" || true
  [[ "$c" == "200" ]] || die "$path expected 200 got $c"
done

echo "[smoke-api] OK"
