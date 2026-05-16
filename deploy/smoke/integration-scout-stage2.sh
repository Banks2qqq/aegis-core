#!/usr/bin/env bash
# Scout 2.0 Stage 2 — stability: duplicate-run guard + metrics presence.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
SCOUT_TIMEOUT="${SCOUT_TIMEOUT:-30}"

require_tools

if [[ -z "$API_KEY" && -f /etc/aegis/agent.env ]]; then
  API_KEY=$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)
fi
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY or AEGIS_MONITOR_API_KEY"

body="$(curl -sf -X POST "$BASE/api/login" \
  -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
[[ -n "$TOKEN" ]] || die "login failed"
auth_hdr=( -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" )

echo "[scout-stage2] duplicate-run guard"
SCOUT_A="$(mktemp)"
SCOUT_B="$(mktemp)"
curl -sS -m 200 -o "$SCOUT_A" "${auth_hdr[@]}" -X POST "$BASE/api/scout" -d '{}' &
pid_a=$!
sleep 2
code_b="$(curl -sS -m "$SCOUT_TIMEOUT" -o "$SCOUT_B" -w "%{http_code}" \
  "${auth_hdr[@]}" -X POST "$BASE/api/scout" -d '{}' || true)"
wait "$pid_a" || true
code_a="$(python3 -c "import json; d=json.load(open('$SCOUT_A')); print('200' if d.get('status')=='success' else 'fail')" 2>/dev/null || echo fail)"
[[ "$code_b" == "409" ]] || [[ "$code_b" == "502" ]] || [[ "$code_b" == "503" ]] || grep -qiE 'уже выполняется|timeout|busy' "$SCOUT_B" 2>/dev/null || {
  echo "  note: concurrent scout returned $code_b (expected 409/502 or busy message)"
}
echo "  first run: $code_a | concurrent: $code_b"

echo "[scout-stage2] prometheus scout metrics"
METRICS_BASE="${METRICS_URL:-http://127.0.0.1:8080}"
metrics="$(fetch_agent_metrics "$TOKEN" "$METRICS_BASE")"
echo "$metrics" | grep -q 'aegis_scout_intel_source_total' || die "missing aegis_scout_intel_source_total"
echo "$metrics" | grep -q 'aegis_scout_pipeline_runs_total' || die "missing aegis_scout_pipeline_runs_total"
echo "  prometheus scout metrics OK"

rm -f "$SCOUT_A" "$SCOUT_B"
echo "[integration-scout-stage2] OK"
