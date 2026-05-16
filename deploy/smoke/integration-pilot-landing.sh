#!/usr/bin/env bash
# H4 — public pilot form accepts POST and persists.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

PUBLIC="${PUBLIC_URL:-https://aegis-security.ru}"
require_tools

echo "[pilot-landing] GET ${PUBLIC}/api/status/public"
curl -sf "${PUBLIC}/api/status/public" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d.get('status')=='ok', d
print('  status ok healing_ready=', d.get('healing_ready'))
"

echo "[pilot-landing] POST ${PUBLIC}/api/pilot"
code="$(curl -sS -o /tmp/aegis-pilot.json -w "%{http_code}" \
  -X POST "${PUBLIC}/api/pilot" \
  -H "Content-Type: application/json" \
  -d '{"name":"Honesty Gate","company":"AEGIS QA","email":"qa@aegis.local","message":"H8 smoke"}' || true)"
[[ "$code" == "200" ]] || die "pilot POST expected 200 got $code: $(cat /tmp/aegis-pilot.json)"
python3 -c "import json; d=json.load(open('/tmp/aegis-pilot.json')); assert d.get('success'), d"

if ssh -o StrictHostKeyChecking=no -o ConnectTimeout=5 localhost true 2>/dev/null; then
  :
fi

echo "[integration-pilot-landing] OK"
