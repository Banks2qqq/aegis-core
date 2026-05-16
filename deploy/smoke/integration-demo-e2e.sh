#!/usr/bin/env bash
# H7 — Demo tour E2E: same chain as /dashboard/demo (status, HITL code-demo, ReAct, audit, air-gap).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY or AEGIS_MONITOR_API_KEY"

REACT_WAIT_SECS="${REACT_WAIT_SECS:-90}"
MISSION="${DEMO_REACT_MISSION:-H7 smoke: safe incident triage with HITL gates and audit trail.}"

require_tools
CURL=(curl -sfS --max-time 120)
body="$("${CURL[@]}" -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
[[ -n "$TOKEN" ]] || die "login failed"
auth=( -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" )

# Ensure connected mode for ReAct LLM unless explicitly testing air-gap only.
if [[ "${DEMO_SKIP_AIRGAP_RESET:-0}" != "1" ]]; then
  "${CURL[@]}" "${auth[@]}" -X POST "$BASE/api/air-gap" -d '{"enabled":false}' >/dev/null 2>&1 || true
fi

echo "[demo-e2e] GET /api/status"
st="$("${CURL[@]}" "${auth[@]}" "$BASE/api/status")"
python3 -c "
import json, sys
d = json.load(sys.stdin)
for k in ('oracle_alive', 'air_gapped', 'react_ready', 'version'):
    assert k in d, f'missing {k}'
print(f'  air_gapped={d[\"air_gapped\"]} react_ready={d[\"react_ready\"]} version={d[\"version\"]}')
" <<<"$st"
AIR_GAP="$(python3 -c "import json,sys; print(json.load(sys.stdin)['air_gapped'])" <<<"$st")"

echo "[demo-e2e] POST /api/code-demo (approved=false → HITL 409)"
code_hitl="$(curl -sS -o /tmp/aegis-code-demo-hitl.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/code-demo" \
  -d '{"task":"Generate a safe Rust IP validator (H7 smoke).","approved":false}' || true)"
[[ "$code_hitl" == "409" ]] || die "code-demo HITL expected 409 got $code_hitl: $(cat /tmp/aegis-code-demo-hitl.json)"
python3 -c "import json; d=json.load(open('/tmp/aegis-code-demo-hitl.json')); assert d.get('status')=='needs_human_approval', d"
echo "  409 needs_human_approval OK"

echo "[demo-e2e] GET /api/react/status"
react_st="$("${CURL[@]}" "${auth[@]}" "$BASE/api/react/status")"
python3 -c "
import json, sys
d = json.load(sys.stdin)
assert isinstance(d, dict)
assert 'react_ready' in d or 'llm_ready' in d, d
print('  react_ready=', d.get('react_ready'), 'llm_ready=', d.get('llm_ready'))
" <<<"$react_st"

export DEMO_REACT_MISSION="$MISSION"
REACT_BODY="$(python3 -c 'import json, os; print(json.dumps({"mission": os.environ["DEMO_REACT_MISSION"]}))')"
agents_before="$("${CURL[@]}" "${auth[@]}" "$BASE/api/agents")"
REACT_BEFORE_TS="$(python3 -c "
import json, sys
for a in json.load(sys.stdin):
    if a.get('id') == 'react':
        print(int(a.get('last_completed_at') or 0))
        break
else:
    print(0)
" <<<"$agents_before")"
echo "[demo-e2e] POST /api/react (react last_completed_at before=$REACT_BEFORE_TS)"
code_react="$(curl -sS -o /tmp/aegis-react.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/react" \
  -d "$REACT_BODY" || true)"
[[ "$code_react" == "200" ]] || die "react expected 200 got $code_react: $(cat /tmp/aegis-react.json)"
python3 -c "import json; d=json.load(open('/tmp/aegis-react.json')); assert d.get('status')=='accepted', d"
echo "  mission accepted"

echo "[demo-e2e] wait for ReAct mission completion (up to ${REACT_WAIT_SECS}s)"
completed=0
export REACT_BEFORE_TS
for _ in $(seq 1 "$((REACT_WAIT_SECS / 5))"); do
  agents="$("${CURL[@]}" "${auth[@]}" "$BASE/api/agents")"
  if python3 -c "
import json, os, sys
before = int(os.environ['REACT_BEFORE_TS'])
agents = json.load(sys.stdin)
for a in agents:
    if a.get('id') == 'react':
        ts = int(a.get('last_completed_at') or 0)
        if ts > before:
            print(ts)
            sys.exit(0)
sys.exit(1)
" <<<"$agents" 2>/dev/null; then
    completed=1
    break
  fi
  sleep 5
done
[[ "$completed" == "1" ]] || die "ReAct mission did not complete within ${REACT_WAIT_SECS}s (check AI_API_KEY / local LLM / air_gapped)"
echo "  ReAct mission completed (new last_completed_at)"

echo "[demo-e2e] GET /api/audit-tail?lines=12"
audit="$("${CURL[@]}" "${auth[@]}" "$BASE/api/audit-tail?lines=12")"
python3 -c "
import json, sys
d = json.load(sys.stdin)
assert d.get('exists') is True, d
lines = d.get('lines') or []
print(f'  audit lines={len(lines)}')
" <<<"$audit"

echo "[demo-e2e] POST /api/air-gap (God Mode toggle + restore)"
TOGGLE_BODY="$(python3 -c "import json; orig='''$AIR_GAP'''.lower() in ('true','1'); print(json.dumps({'enabled': not orig}))")"
RESTORE_BODY="$(python3 -c "import json; orig='''$AIR_GAP'''.lower() in ('true','1'); print(json.dumps({'enabled': orig}))")"
"${CURL[@]}" "${auth[@]}" -X POST "$BASE/api/air-gap" -d "$TOGGLE_BODY" >/tmp/aegis-airgap-toggle.json
python3 -c "import json; d=json.load(open('/tmp/aegis-airgap-toggle.json')); assert d.get('success'), d"
"${CURL[@]}" "${auth[@]}" -X POST "$BASE/api/air-gap" -d "$RESTORE_BODY" >/tmp/aegis-airgap-restore.json
python3 -c "import json; d=json.load(open('/tmp/aegis-airgap-restore.json')); assert d.get('air_gapped') in (True, False), d"
echo "  air-gap toggle restored"

echo "[demo-e2e] POST /api/code-demo (approved=true → 200)"
code_ok="$(curl -sS -o /tmp/aegis-code-demo-ok.json -w "%{http_code}" \
  "${auth[@]}" -X POST "$BASE/api/code-demo" \
  -d '{"task":"H7 approved code demo snippet.","approved":true}' || true)"
[[ "$code_ok" == "200" ]] || die "code-demo approved expected 200 got $code_ok: $(cat /tmp/aegis-code-demo-ok.json)"
python3 -c "import json; d=json.load(open('/tmp/aegis-code-demo-ok.json')); assert d.get('status')=='accepted', d"
echo "  code-demo accepted"

echo "[demo-e2e] GET /api/agents"
agents="$("${CURL[@]}" "${auth[@]}" "$BASE/api/agents")"
python3 -c "import json,sys; a=json.load(sys.stdin); assert isinstance(a,list) and len(a)>=1; print('  agents count=', len(a))" <<<"$agents"

echo "[integration-demo-e2e] OK"
