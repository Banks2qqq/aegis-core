#!/usr/bin/env bash
# C2 — safe-surf.ru / НКЦКИ RSS in scout hub.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
METRICS_BASE="${METRICS_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
auth=( -H "Authorization: Bearer $TOKEN" )

echo "[scout-c2] POST /api/scout (safe_surf / НКЦКИ)"
SCOUT_TMP="$(mktemp)"
code="$(curl -sS -m 200 -o "$SCOUT_TMP" -w "%{http_code}" \
  "${auth[@]}" -H "Content-Type: application/json" -X POST "$BASE/api/scout" -d '{}' || true)"
[[ "$code" == "200" ]] || die "scout returned $code: $(head -c 300 "$SCOUT_TMP")"

python3 - "$SCOUT_TMP" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
sources = d.get("sources") or []
by_id = {s.get("id"): s for s in sources if isinstance(s, dict)}
s = by_id.get("safe_surf")
if not s:
    raise SystemExit("missing safe_surf in scout sources")
st = s.get("status", "")
if st not in ("ok", "skipped"):
    raise SystemExit(f"safe_surf unexpected status {st}: {s.get('note')}")
if st == "ok" and int(s.get("count") or 0) < 1:
    raise SystemExit("safe_surf ok but count=0")
print(f"  safe_surf: status={st} count={s.get('count')} note={(s.get('note') or '')[:100]}")
print("  sources_ok=", d.get("sources_ok"), "total_findings=", d.get("total_findings"))
PY

metrics="$(fetch_agent_metrics "$TOKEN" "$METRICS_BASE")"
echo "$metrics" | grep -q 'source="safe_surf"' || die "missing safe_surf prometheus metric"
echo "[scout-c2] prometheus safe_surf OK"

rm -f "$SCOUT_TMP"
echo "[integration-scout-c2] OK"
