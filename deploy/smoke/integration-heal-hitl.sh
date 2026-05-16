#!/usr/bin/env bash
# H3 — HITL heal: full orchestrator run → pending → approve/reject → metrics.
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

echo "[heal-hitl] POST /api/heal/run (patch_type=custom → HITL)"
code="$(curl -sS -o /tmp/aegis-heal-run.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/heal/run" \
  -d '{"anomaly":"integration HITL: elevated privilege anomaly on admin host","patch_type":"custom"}' || true)"
[[ "$code" == "200" ]] || die "heal/run expected 200 got $code: $(cat /tmp/aegis-heal-run.json)"
python3 -c "
import json,sys
d=json.load(open('/tmp/aegis-heal-run.json'))
assert d.get('pending_hitl') is True, d
print('  pending_hitl=True patch_id=', d['result']['patch_id'][:36])
"
PATCH_ID="$(python3 -c "import json; print(json.load(open('/tmp/aegis-heal-run.json'))['result']['patch_id'])")"

echo "[heal-hitl] GET /api/heal/pending"
pending="$(curl -sf "${auth[@]}" "$BASE/api/heal/pending")"
echo "$pending" | python3 -c "
import json,sys
d=json.load(sys.stdin)
ids=[i['patch_id'] for i in d.get('items',[])]
assert '${PATCH_ID}' in ids, d
print('  pending count=', d.get('count'))
"

echo "[heal-hitl] POST /api/heal/approve"
code_a="$(curl -sS -o /tmp/aegis-heal-approve.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/heal/approve" \
  -d "{\"patch_id\":\"$PATCH_ID\",\"note\":\"integration-hitl\"}" || true)"
[[ "$code_a" == "200" ]] || die "approve expected 200 got $code_a"

echo "[heal-hitl] POST /api/heal/run + reject"
code_r="$(curl -sS -o /tmp/aegis-heal-run2.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/heal/run" \
  -d '{"anomaly":"integration reject path","patch_type":"code"}' || true)"
[[ "$code_r" == "200" ]] || die "heal/run 2 failed"
PID2="$(python3 -c "import json; print(json.load(open('/tmp/aegis-heal-run2.json'))['result']['patch_id'])")"
curl -sf "${auth[@]}" -X POST "$BASE/api/heal/reject" \
  -d "{\"patch_id\":\"$PID2\",\"reason\":\"integration-reject\"}" >/dev/null

echo "[heal-hitl] prometheus"
metrics="$(fetch_agent_metrics "$TOKEN" "${METRICS_URL:-http://127.0.0.1:8080}")"
echo "$metrics" | grep -q 'aegis_heal_hitl_total' || die "missing aegis_heal_hitl_total"
echo "$metrics" | grep -q 'action="approved"' || die "missing approved"
echo "$metrics" | grep -q 'action="rejected"' || die "missing rejected"

echo "[integration-heal-hitl] OK"
