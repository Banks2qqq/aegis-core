#!/usr/bin/env bash
# Scout honest 10/10: structured report in /api/scout response.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2- | tr -d '"')}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
echo "==> Scout honest 10/10 smoke"
echo "    BASE=${BASE}"

body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
[[ -n "$TOKEN" ]] || die "login failed"
auth=( -H "Authorization: Bearer $TOKEN" )

if [[ "${SCOUT_CHECK_CONCURRENT:-0}" == "1" ]]; then
  curl -sS -o /dev/null -m 3 \
    "${auth[@]}" -H "Content-Type: application/json" -X POST "$BASE/api/scout" -d '{}' &
  sleep 0.5
  code2="$(curl -sS -o /dev/null -w "%{http_code}" \
    "${auth[@]}" -H "Content-Type: application/json" -X POST "$BASE/api/scout" -d '{}' \
    --max-time 3 2>/dev/null || true)"
  [[ "$code2" == "409" ]] || die "expected 409 on concurrent scout, got $code2"
  echo "PASS: concurrent SCOUT returns 409"
  echo "    waiting for in-flight scout (up to 200s)..."
  sleep 120
fi

SCOUT_TMP="$(mktemp)"
trap 'rm -f "$SCOUT_TMP"' EXIT
WAIT="${SCOUT_WAIT_SECS:-200}"
echo "[scout-honest-10] POST /api/scout (wait up to ${WAIT}s)"
code="$(curl -sS -m "$WAIT" -o "$SCOUT_TMP" -w "%{http_code}" \
  "${auth[@]}" -H "Content-Type: application/json" -X POST "$BASE/api/scout" -d '{}' || true)"
[[ "$code" == "200" ]] || die "scout returned $code: $(head -c 500 "$SCOUT_TMP")"

python3 - "$SCOUT_TMP" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert d.get("status") == "success", d
report = d.get("report")
assert report, "missing report object"
assert report.get("executive_summary_ru"), "missing executive_summary_ru"
assert len(report.get("top_findings", [])) >= 1, "top_findings empty"
assert "autonomy" in report and report["autonomy"].get("description_ru"), "autonomy policy"
enrich = report.get("enrichment") or {}
for k in ("total_iocs", "total_cves"):
    assert k in enrich, f"enrichment.{k}"
sources = d.get("sources") or []
by_id = {s.get("id"): s for s in sources if isinstance(s, dict)}
ok_sources = [s for s in sources if s.get("status") == "ok"]
assert len(ok_sources) >= 6, f"sources_ok count {len(ok_sources)} < 6"
for want in ("pt_analytics", "bi_zone", "facct", "rt_solar"):
    assert want in by_id, f"missing phase4 source {want}"
    st = by_id[want].get("status")
    assert st in ("ok", "skipped"), f"{want} status={st}"
pt = by_id["pt_analytics"]
if pt.get("status") == "ok":
    assert int(pt.get("count") or 0) >= 1, "pt_analytics ok but count=0"
print(
    f"PASS: findings={d.get('found')} sources_ok={len(ok_sources)} "
    f"top={len(report['top_findings'])} reactions={len(report.get('reactions', []))} "
    f"pt_analytics={pt.get('status')} count={pt.get('count')}"
)
PY

echo "=== integration-scout-honest-10: PASS ==="
