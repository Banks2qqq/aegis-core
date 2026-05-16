#!/usr/bin/env bash
# Run full federation prod smoke ON the primary VPS (has mTLS certs in /etc/aegis/federation).
set -euo pipefail
PRIMARY_URL="${PRIMARY_URL:-https://aegis-security.ru}"
if [[ -z "${API_KEY:-}" && -f /etc/aegis/agent.env ]]; then
  API_KEY=$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)
fi
API_KEY="${API_KEY:-${SMOKE_API_KEY:-test-key-enterprise}}"
FED_SECRET="${FEDERATION_SHARED_SECRET:-$(grep '^FEDERATION_SHARED_SECRET=' /etc/aegis/agent.env 2>/dev/null | cut -d= -f2-)}"
CERT_DIR="${FED_CERT_DIR:-/etc/aegis/federation}"

[[ -n "$FED_SECRET" ]] || { echo "FEDERATION_SHARED_SECRET missing" >&2; exit 1; }

echo "[fed-vps] health"
curl -sfS "$PRIMARY_URL/health" | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='ok'"
curl -sfS "https://node2.aegis-security.ru/health" | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='ok'"

TOK=$(curl -sfS -X POST "$PRIMARY_URL/api/login" -H "Content-Type: application/json" \
  -d "{\"api_key\":\"$API_KEY\"}" | python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])")

echo "[fed-vps] federation health"
curl -sfS -H "Authorization: Bearer $TOK" "$PRIMARY_URL/api/federation/health" | python3 -m json.tool | head -25

echo "[fed-vps] sync all"
curl -sfS -H "Authorization: Bearer $TOK" -H "Content-Type: application/json" \
  -X POST "$PRIMARY_URL/api/federation/sync" -d '{"sync_all":true}' \
  | python3 -c "import sys,json; r=json.load(sys.stdin); assert r.get('success')"

echo "[fed-vps] mTLS handshake node2:8443"
openssl s_client -connect node2.aegis-security.ru:8443 -servername node2.aegis-security.ru \
  -cert "${CERT_DIR}/primary.client.pem" -key "${CERT_DIR}/primary.client.key" </dev/null 2>/dev/null \
  | grep -q "Verify return code: 0"

echo "[fed-vps] federation API merkle"
curl -sfS --cert "${CERT_DIR}/primary.client.pem" --key "${CERT_DIR}/primary.client.key" \
  -H "X-AEGIS-Federation-Token: $FED_SECRET" \
  "https://node2.aegis-security.ru:8443/federation/merkle" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert d.get('merkle_root') or d.get('root')"

echo "[fed-vps] metrics API"
curl -sfS -H "Authorization: Bearer $TOK" "$PRIMARY_URL/api/federation/metrics" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert d.get('peers')"

echo "[fed-vps] OK"
