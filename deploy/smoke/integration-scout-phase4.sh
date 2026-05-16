#!/usr/bin/env bash
# Scout phase 4: RU blog sources registered + MITRE DR path optional.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2- | tr -d '"')}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
echo "==> Scout phase 4 smoke (BASE=${BASE})"

body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
auth=( -H "Authorization: Bearer $TOKEN" )

SCOUT_TMP="$(mktemp)"
trap 'rm -f "$SCOUT_TMP"' EXIT
code="$(curl -sS -m "${SCOUT_WAIT_SECS:-200}" -o "$SCOUT_TMP" -w "%{http_code}" \
  "${auth[@]}" -H "Content-Type: application/json" -X POST "$BASE/api/scout" -d '{}' || true)"
[[ "$code" == "200" ]] || die "scout HTTP $code: $(head -c 400 "$SCOUT_TMP")"

python3 - "$SCOUT_TMP" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert d.get("status") == "success"
sources = {s.get("id"): s for s in (d.get("sources") or []) if isinstance(s, dict)}
for want in ("pt_analytics", "bi_zone", "facct", "rt_solar"):
    if want not in sources:
        raise SystemExit(f"missing source id {want} in scout response")
    st = sources[want].get("status")
    if st not in ("ok", "skipped"):
        raise SystemExit(f"{want} unexpected status {st}: {sources[want].get('note')}")
pt = sources["pt_analytics"]
if pt.get("status") == "ok" and int(pt.get("count") or 0) < 1:
    raise SystemExit("pt_analytics ok but count=0")
ok_n = sum(1 for s in sources.values() if s.get("status") == "ok")
assert ok_n >= 7, f"expected >=7 ok sources, got {ok_n}"
print(f"PASS: phase4 sources present; pt_analytics={pt.get('status')} count={pt.get('count')} total_ok={ok_n}")
PY

echo "=== integration-scout-phase4: PASS ==="
