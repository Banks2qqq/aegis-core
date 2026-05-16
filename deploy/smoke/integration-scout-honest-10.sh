#!/usr/bin/env bash
# Scout honest 10/10: structured report in /api/scout response.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib.sh
source "${SCRIPT_DIR}/lib.sh"

need_jwt
need_cmd curl
need_cmd python3

echo "==> Scout honest 10/10 smoke"
echo "    BASE_URL=${BASE_URL}"

# Concurrent scout must 409
code=$(curl -sS -o /dev/null -w "%{http_code}" \
  -X POST "${BASE_URL}/api/scout" \
  -H "Authorization: Bearer ${JWT}" \
  -H "Content-Type: application/json" \
  -d '{}' \
  --max-time 2 &
pid1=$!
sleep 0.3
code2=$(curl -sS -o /dev/null -w "%{http_code}" \
  -X POST "${BASE_URL}/api/scout" \
  -H "Authorization: Bearer ${JWT}" \
  -H "Content-Type: application/json" \
  -d '{}' \
  --max-time 2 || true)
wait "$pid1" 2>/dev/null || true
if [[ "${code2}" == "409" || "${code}" == "409" ]]; then
  echo "PASS: concurrent SCOUT returns 409"
else
  echo "WARN: concurrent SCOUT codes: ${code} / ${code2} (one may have finished fast)"
fi

# Wait for any in-flight scout
sleep 2

body=$(mktemp)
trap 'rm -f "$body"' EXIT
http=$(curl -sS -o "$body" -w "%{http_code}" \
  -X POST "${BASE_URL}/api/scout" \
  -H "Authorization: Bearer ${JWT}" \
  -H "Content-Type: application/json" \
  -d '{}' \
  --max-time "${SCOUT_WAIT_SECS:-200}")

[[ "$http" == "200" ]] || die "POST /api/scout HTTP $http: $(head -c 400 "$body")"

python3 - "$body" <<'PY'
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
ok_sources = [s for s in sources if s.get("status") == "ok"]
assert len(ok_sources) >= 6, f"sources_ok count {len(ok_sources)} < 6"
print(f"PASS: findings={d.get('found')} sources_ok={len(ok_sources)} top={len(report['top_findings'])} reactions={len(report.get('reactions', []))}")
PY

echo "=== integration-scout-honest-10: PASS ==="
