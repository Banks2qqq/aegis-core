#!/usr/bin/env bash
# Scout 2.0 autonomy smoke — multi-source collect + critic + optional heal queue.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
SCOUT_TIMEOUT="${SCOUT_TIMEOUT:-200}"
RUN_SCOUT="${RUN_SCOUT:-1}"

require_tools

if [[ -z "$API_KEY" && -f /etc/aegis/agent.env ]]; then
  API_KEY=$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)
fi
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY or AEGIS_MONITOR_API_KEY in agent.env"

body="$(curl -sf -X POST "$BASE/api/login" \
  -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
[[ -n "$TOKEN" ]] || die "login failed"

auth_hdr=( -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" )

echo "[scout-autonomy] health"
curl -sf "$BASE/health" >/dev/null

if [[ "$RUN_SCOUT" != "1" ]]; then
  echo "[scout-autonomy] SKIP_SCOUT=1 — health/login only"
  exit 0
fi

echo "[scout-autonomy] POST /api/scout (timeout ${SCOUT_TIMEOUT}s)"
SCOUT_TMP="$(mktemp)"
code="$(curl -sS -m "$SCOUT_TIMEOUT" -o "$SCOUT_TMP" -w "%{http_code}" \
  "${auth_hdr[@]}" -X POST "$BASE/api/scout" -d '{}' || true)"
[[ "$code" == "200" ]] || die "/api/scout expected 200 got $code body=$(head -c 400 "$SCOUT_TMP")"

python3 - "$SCOUT_TMP" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert d.get("status") == "success", d
found = int(d.get("total_findings") or d.get("found") or 0)
assert found > 0, f"expected findings > 0 got {found}"
sources_ok = int(d.get("sources_ok") or 0)
assert sources_ok >= 1, f"expected sources_ok >= 1 got {sources_ok}"
print(f"  findings={found} sources_ok={sources_ok} skipped={d.get('sources_skipped',0)} "
      f"critic={d.get('critic_verdict')} risk={d.get('critic_risk')} heal_q={d.get('healing_attempted')}")
PY
rm -f "$SCOUT_TMP"

echo "[integration-scout-autonomy] OK"
