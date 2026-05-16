#!/usr/bin/env bash
# Secondary node: local agent health + federation port; notify via shared Telegram env if present.
set -euo pipefail
LOG_TAG="aegis-federation-alert-secondary"
NODE_URL="${NODE_URL:-https://node2.aegis-security.ru}"
CERT_DIR="${FED_CERT_DIR:-/etc/aegis/federation}"

[[ -f /etc/aegis/federation-alert.env ]] && set -a && source /etc/aegis/federation-alert.env && set +a

notify() {
  local msg="[node2] $1"
  logger -t "$LOG_TAG" "$msg"
  if [[ -n "${ALERT_TELEGRAM_BOT_TOKEN:-}" && -n "${ALERT_TELEGRAM_CHAT_ID:-}" ]]; then
    curl -sfS --max-time 20 -X POST "https://api.telegram.org/bot${ALERT_TELEGRAM_BOT_TOKEN}/sendMessage" \
      -d "chat_id=${ALERT_TELEGRAM_CHAT_ID}" --data-urlencode "text=${msg}" >/dev/null 2>&1 || true
  fi
}

issues=()
curl -sfS "${NODE_URL}/health" >/dev/null || issues+=("health endpoint down")
cert="${FED_CLIENT_CERT:-}"
key="${FED_CLIENT_KEY:-}"
[[ -z "$cert" && -f "${CERT_DIR}/secondary.client.pem" ]] && cert="${CERT_DIR}/secondary.client.pem" && key="${CERT_DIR}/secondary.client.key"
[[ -z "$cert" && -f "${CERT_DIR}/primary.client.pem" ]] && cert="${CERT_DIR}/primary.client.pem" && key="${CERT_DIR}/primary.client.key"
if [[ -n "$cert" && -f "$key" ]]; then
  openssl s_client -connect localhost:8443 -servername node2.aegis-security.ru \
    -cert "$cert" -key "$key" </dev/null 2>/dev/null \
    | grep -q "Verify return code: 0" || issues+=("local mTLS :8443 handshake failed")
fi
systemctl is-active --quiet aegis-agent || issues+=("aegis-agent not active")

if [[ ${#issues[@]} -gt 0 ]]; then
  notify "$(IFS='; '; echo "${issues[*]}")"
  exit 1
fi
logger -t "$LOG_TAG" "OK"
exit 0
