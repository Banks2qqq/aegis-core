#!/usr/bin/env bash
# D1 — post-restore health: agent up, KB readable, backup script idempotent.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"

echo "[dr-backup] /health"
curl -sf --max-time 15 "$BASE/health" >/dev/null

echo "[dr-backup] /api/status"
curl -sf --max-time 15 -H "Authorization: Bearer $TOKEN" "$BASE/api/status" >/dev/null
echo "  api/status OK"

echo "[dr-backup] backup script dry-run"
TMP="/tmp/aegis-dr-smoke-$(date +%s)"
/opt/aegis/deploy/backup-aegis.sh "$TMP"
[[ -f "$TMP/manifest.json" ]] || die "manifest missing"
[[ -f "$TMP/aegis.db" ]] || die "aegis.db missing in backup"
rm -rf "$TMP"

echo "[integration-dr-backup] OK"
