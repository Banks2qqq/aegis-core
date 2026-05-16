#!/usr/bin/env bash
# H1 — real Docker sandbox: API verify + metrics + negative case.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
auth=( -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" )

echo "[heal-sandbox-real] POST /api/heal/sandbox-verify (good patch)"
code_good="$(curl -sS -o /tmp/aegis-sandbox-good.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/heal/sandbox-verify" -d '{}' || true)"
[[ "$code_good" == "200" ]] || die "good patch expected 200 got $code_good: $(cat /tmp/aegis-sandbox-good.json)"
dur_good="$(python3 -c "import json; d=json.load(open('/tmp/aegis-sandbox-good.json')); print(d.get('duration_secs',0))")"
python3 -c "import sys; d=float('${dur_good}'); sys.exit(0 if d>0 else 1)" || die "duration_secs should be > 0 got $dur_good"
echo "  passed duration_secs=$dur_good runtime=$(python3 -c "import json;print(json.load(open('/tmp/aegis-sandbox-good.json'))['runtime'])")"

echo "[heal-sandbox-real] POST /api/heal/sandbox-verify (denylisted patch)"
bad_patch='{"patch":"HEALING PATCH [Config]\nrm -rf /\n"}'
code_bad="$(curl -sS -o /tmp/aegis-sandbox-bad.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/heal/sandbox-verify" -d "$bad_patch" || true)"
[[ "$code_bad" == "422" ]] || die "bad patch expected 422 got $code_bad: $(cat /tmp/aegis-sandbox-bad.json)"
echo "  rejected as expected"

echo "[heal-sandbox-real] prometheus metrics"
metrics="$(fetch_agent_metrics "$TOKEN" "${METRICS_URL:-http://127.0.0.1:8080}")"
echo "$metrics" | grep -q 'aegis_healing_sandbox_result_total' || die "missing aegis_healing_sandbox_result_total"
echo "$metrics" | grep -q 'runtime="docker"' || die "missing docker runtime label"
echo "$metrics" | grep -q 'result="pass"' || die "missing pass result"
echo "$metrics" | grep -q 'result="fail"' || die "missing fail result"

echo "[integration-heal-sandbox-real] OK"
