#!/usr/bin/env bash
# H2 — real Docker deception listener: deploy, HTTP canary, trip, metrics.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
PORT="${AEGIS_DECEPTION_SMOKE_PORT:-$((19100 + RANDOM % 800))}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
auth=( -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" )

echo "[deception-h2] POST /api/deception/deploy port=$PORT"
code="$(curl -sS -o /tmp/aegis-deception-deploy.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/deception/deploy" \
  -d "{\"port\":$PORT}" || true)"
[[ "$code" == "200" ]] || die "deploy expected 200 got $code: $(cat /tmp/aegis-deception-deploy.json)"
python3 -c "
import json, sys
d=json.load(open('/tmp/aegis-deception-deploy.json'))
assert d.get('runtime')=='docker', d
assert d.get('http_ok') is True, d
print('  runtime=docker http_ok=True')
"

canary="$(python3 -c "import json; print(json.load(open('/tmp/aegis-deception-deploy.json'))['canary'])")"
resp="$(curl -sf "http://127.0.0.1:${PORT}/" || die "curl listener failed")"
echo "$resp" | grep -qF "$canary" || die "canary not in HTTP body"

echo "[deception-h2] POST /api/deception/canary-trip"
code_trip="$(curl -sS -o /tmp/aegis-deception-trip.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/deception/canary-trip" \
  -d "{\"token\":\"$canary\",\"source\":\"integration-h2\"}" || true)"
[[ "$code_trip" == "200" ]] || die "canary-trip expected 200 got $code_trip"

echo "[deception-h2] prometheus metrics"
metrics="$(fetch_agent_metrics "$TOKEN" "${METRICS_URL:-http://127.0.0.1:8080}")"
echo "$metrics" | grep -q 'aegis_deception_listener_total' || die "missing aegis_deception_listener_total"
echo "$metrics" | grep -q 'runtime="docker"' || die "missing docker runtime label"
echo "$metrics" | grep -q 'result="pass"' || die "missing pass result"

echo "[integration-deception-h2] OK"
