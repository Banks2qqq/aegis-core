#!/usr/bin/env bash
# B1 — heal apply smoke: POST /api/heal/smoke → applied (staging) or dry_run (prod).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
EXPECT_APPLY="${EXPECT_HEAL_APPLY:-0}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
auth=( -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" )

echo "[heal-apply] POST /api/heal/smoke (expect_apply=$EXPECT_APPLY)"
resp="$(curl -sf "${auth[@]}" -X POST "$BASE/api/heal/smoke" -d '{}')"
mode="$(python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('mode',''))" <<<"$resp")"
applied="$(python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('applied',False))" <<<"$resp")"
patch_id="$(python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('patch_id',''))" <<<"$resp")"

if [[ "$EXPECT_APPLY" == "1" ]]; then
  [[ "$mode" == "applied" ]] || die "expected mode=applied got mode=$mode body=$resp"
  [[ "$applied" == "True" || "$applied" == "true" ]] || die "expected applied=true got $applied"
  if [[ -f /etc/aegis/agent.env ]]; then
    patch_glob="/opt/aegis/backend/data/healing/applied/${patch_id}.patch"
    [[ -f "$patch_glob" ]] || die "patch file missing: $patch_glob"
    echo "  patch file on disk: $patch_glob"
  fi
  echo "  heal apply OK patch_id=$patch_id"
else
  [[ "$mode" == "dry_run" ]] || die "expected mode=dry_run got mode=$mode body=$resp"
  echo "  heal dry_run OK (production policy) patch_id=$patch_id"
fi

echo "[integration-heal-apply] OK"
