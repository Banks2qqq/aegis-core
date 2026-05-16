#!/usr/bin/env bash
# H6 — nginx /metrics proxy returns Prometheus text with JWT.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

PUBLIC_URL="${PUBLIC_URL:-https://aegis-security.ru}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
body="$(curl -sf -X POST "${PUBLIC_URL}/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"

echo "[nginx-h6] GET ${PUBLIC_URL}/metrics"
metrics="$(curl -sf -H "Authorization: Bearer $TOKEN" "${PUBLIC_URL}/metrics")"
echo "$metrics" | head -5
echo "$metrics" | grep -q '^# HELP aegis_' || die "not prometheus format from nginx /metrics"
echo "$metrics" | grep -q 'aegis_heal_hitl_total' || echo "  (warn: aegis_heal_hitl_total not in scrape yet)"

echo "[nginx-h6] GET ${PUBLIC_URL}/api/status/public"
curl -sf "${PUBLIC_URL}/api/status/public" | python3 -c "import json,sys; d=json.load(sys.stdin); assert d.get('status')=='ok', d"

echo "[integration-nginx-metrics-h6] OK"
