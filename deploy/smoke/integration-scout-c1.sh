#!/usr/bin/env bash
# C1 — Talos + FortiGuard sources present in scout intel hub.
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

echo "[scout-c1] POST /api/scout (intel hub — talos + fortiguard)"
SCOUT_TMP="$(mktemp)"
code="$(curl -sS -m 200 -o "$SCOUT_TMP" -w "%{http_code}" \
  "${auth[@]}" -H "Content-Type: application/json" -X POST "$BASE/api/scout" -d '{}' || true)"
[[ "$code" == "200" ]] || die "scout returned $code: $(head -c 300 "$SCOUT_TMP")"

python3 - "$SCOUT_TMP" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
sources = d.get("sources") or []
by_id = {s.get("id"): s for s in sources if isinstance(s, dict)}
for want in ("talos", "fortiguard"):
    s = by_id.get(want)
    if not s:
        raise SystemExit(f"missing source {want} in scout response")
    st = s.get("status", "")
    if want == "fortiguard" and st != "ok":
        raise SystemExit(f"fortiguard expected ok, got {st}: {s.get('note')}")
    if want == "talos" and st not in ("ok", "skipped"):
        raise SystemExit(f"talos unexpected status {st}")
    if want == "talos" and st == "ok" and int(s.get("count") or 0) < 1:
        raise SystemExit("talos ok but count=0")
    print(f"  {want}: status={st} count={s.get('count')} note={s.get('note','')[:80]}")
print("scout sources_ok=", d.get("sources_ok"), "total_findings=", d.get("total_findings"))
PY

metrics="$(fetch_agent_metrics "$TOKEN" "$METRICS_BASE")"
echo "$metrics" | grep -q 'aegis_scout_intel_source_total{source="fortiguard"' || \
  echo "$metrics" | grep -q 'source="fortiguard"' || die "missing fortiguard prometheus metric"
echo "[scout-c1] prometheus fortiguard metric OK"

rm -f "$SCOUT_TMP"
echo "[integration-scout-c1] OK"
