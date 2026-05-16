#!/usr/bin/env bash
# Periodic federation health check — log + optional Telegram alert.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -f "${SCRIPT_DIR}/../smoke/lib.sh" ]]; then
  ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
elif [[ -f /opt/aegis/deploy/smoke/lib.sh ]]; then
  ROOT="/opt/aegis"
else
  ROOT="${AEGIS_ROOT:-$(cd "${SCRIPT_DIR}/../.." 2>/dev/null && pwd)}"
fi
# shellcheck source=../smoke/lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

PRIMARY_URL="${PRIMARY_URL:-https://aegis-security.ru}"
# On-node checks use loopback to avoid nginx rate limits on /api/login
AGENT_URL="${AGENT_URL:-http://127.0.0.1:8080}"
if [[ -z "${API_KEY:-}" && -f /etc/aegis/agent.env ]]; then
  API_KEY=$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)
fi
API_KEY="${API_KEY:-${SMOKE_API_KEY:-test-key-enterprise}}"
ALERT_SYNC_LATENCY_MS="${ALERT_SYNC_LATENCY_MS:-30000}"
STATE_FILE="${STATE_FILE:-/var/lib/aegis/federation-alert-state}"
LOG_TAG="aegis-federation-alert"

ALERT_ENV_FILE="${ALERT_ENV_FILE:-/etc/aegis/federation-alert.env}"

mkdir -p "$(dirname "$STATE_FILE")"
require_tools

telegram_resolve_chat() {
  [[ -n "${ALERT_TELEGRAM_CHAT_ID:-}" ]] && return 0
  [[ -n "${ALERT_TELEGRAM_BOT_TOKEN:-}" ]] || return 1
  local cid json
  json=$(curl -sfS --max-time 20 \
    "https://api.telegram.org/bot${ALERT_TELEGRAM_BOT_TOKEN}/getUpdates?limit=30" 2>/dev/null) || return 1
  cid=$(printf '%s' "$json" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for u in reversed(d.get('result', [])):
    m = u.get('message') or u.get('edited_message') or u.get('channel_post') or {}
    c = m.get('chat', {}).get('id')
    if c is not None:
        print(c)
        break
" 2>/dev/null) || true
  [[ -n "$cid" ]] || return 1
  export ALERT_TELEGRAM_CHAT_ID="$cid"
  if [[ -w "$ALERT_ENV_FILE" ]]; then
    grep -v '^ALERT_TELEGRAM_CHAT_ID=' "$ALERT_ENV_FILE" >"${ALERT_ENV_FILE}.tmp" 2>/dev/null || true
    echo "ALERT_TELEGRAM_CHAT_ID=${cid}" >>"${ALERT_ENV_FILE}.tmp"
    chmod 600 "${ALERT_ENV_FILE}.tmp"
    mv "${ALERT_ENV_FILE}.tmp" "$ALERT_ENV_FILE"
    logger -t "$LOG_TAG" "Telegram chat_id saved: ${cid}"
  fi
  return 0
}

notify() {
  local msg=$1
  logger -t "$LOG_TAG" "$msg"
  echo "[alert] $msg"
  telegram_resolve_chat || true
  if [[ -n "${ALERT_TELEGRAM_BOT_TOKEN:-}" && -n "${ALERT_TELEGRAM_CHAT_ID:-}" ]]; then
    curl -sfS --max-time 20 -X POST "https://api.telegram.org/bot${ALERT_TELEGRAM_BOT_TOKEN}/sendMessage" \
      -d "chat_id=${ALERT_TELEGRAM_CHAT_ID}" \
      --data-urlencode "text=${msg}" >/dev/null 2>&1 || \
      logger -t "$LOG_TAG" "Telegram send failed"
  elif [[ -n "${ALERT_TELEGRAM_BOT_TOKEN:-}" ]]; then
    logger -t "$LOG_TAG" "Telegram: no chat_id — send /start to @AEGIS_GOD_BOT"
  fi
}

should_alert() {
  local key=$1 now last=0
  now=$(date +%s)
  [[ -f "$STATE_FILE" ]] && last=$(grep "^${key}=" "$STATE_FILE" 2>/dev/null | tail -1 | cut -d= -f2 || echo 0)
  if [[ $((now - last)) -ge 300 ]]; then
    echo "${key}=${now}" >> "$STATE_FILE"
    return 0
  fi
  return 1
}

JWT_CACHE="${JWT_CACHE:-/var/lib/aegis/alert-jwt}"
mkdir -p "$(dirname "$JWT_CACHE")"

fetch_jwt() {
  curl -sfS -X POST "${AGENT_URL}/api/login" \
    -H "Content-Type: application/json" \
    -d "{\"api_key\":\"${API_KEY}\"}" \
    | python3 -c "import sys,json; print(json.load(sys.stdin).get('access_token',''))" 2>/dev/null
}

TOK=""
[[ -f "$JWT_CACHE" ]] && TOK=$(cat "$JWT_CACHE" 2>/dev/null || true)
if [[ -n "$TOK" ]] && curl -sfS -H "Authorization: Bearer ${TOK}" \
  "${AGENT_URL}/api/federation/health" >/dev/null 2>&1; then
  :
else
  TOK=$(fetch_jwt) || {
    notify "CRITICAL: cannot login to agent (${AGENT_URL})"
    exit 2
  }
  [[ -n "$TOK" ]] || { notify "CRITICAL: empty JWT"; exit 2; }
  printf '%s' "$TOK" >"$JWT_CACHE"
  chmod 600 "$JWT_CACHE"
fi

HEALTH=$(curl -sfS -H "Authorization: Bearer ${TOK}" "${AGENT_URL}/api/federation/health" 2>/dev/null) || {
  notify "CRITICAL: federation health API unreachable"
  exit 2
}

curl -sfS "${PRIMARY_URL}/health" >/dev/null || {
  should_alert public_health_fail && notify "CRITICAL: public ${PRIMARY_URL}/health down"
}

check_mtls_local() {
  local host=$1 cert_dir="${FED_CERT_DIR:-/etc/aegis/federation}"
  local cert="${FED_CLIENT_CERT:-${cert_dir}/primary.client.pem}"
  local key="${FED_CLIENT_KEY:-${cert_dir}/primary.client.key}"
  [[ -f "$cert" && -f "$key" ]] || return 0
  openssl s_client -connect "${host}:8443" -servername "$host" \
    -cert "$cert" -key "$key" </dev/null 2>/dev/null \
    | grep -q "Verify return code: 0"
}

if ! check_mtls_local "node2.aegis-security.ru"; then
  should_alert mtls_port_fail && notify "Federation mTLS handshake FAILED (node2:8443)"
fi

ISSUES=$(python3 -c "
import json, os
h = json.loads('''${HEALTH}''')
issues = []
for p in (h.get('report') or {}).get('peers') or []:
    pid = p.get('id', '?')
    st = p.get('status', 'offline')
    ready = p.get('federation_ready', False)
    err = (p.get('error') or '')[:120]
    if st == 'offline' or not p.get('online'):
        issues.append(f'peer {pid} OFFLINE: {err}')
    elif not ready:
        issues.append(f'peer {pid} not ready (status={st}): {err}')
    lat = p.get('last_sync_duration_ms')
    if lat and int(lat) > int(os.environ.get('ALERT_SYNC_LATENCY_MS', '30000')):
        issues.append(f'peer {pid} slow sync {lat}ms')
print(chr(10).join(issues))
")

if [[ -n "$ISSUES" ]]; then
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    key=$(printf '%s' "$line" | shasum | cut -c1-12)
    should_alert "$key" && notify "AEGIS Federation: ${line}"
  done <<<"$ISSUES"
  exit 1
fi

if [[ "${ALERT_TEST_NOTIFY:-}" == "1" ]]; then
  notify "AEGIS Federation: test OK — alerts configured"
fi

logger -t "$LOG_TAG" "OK: federation healthy"
exit 0
