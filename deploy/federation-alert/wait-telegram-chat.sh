#!/usr/bin/env bash
# Poll getUpdates until a user messages the bot (run on VPS after token is configured).
set -euo pipefail
BOT_TOKEN="${ALERT_TELEGRAM_BOT_TOKEN:?Set ALERT_TELEGRAM_BOT_TOKEN}"
ENV_FILE="${ALERT_ENV_FILE:-/etc/aegis/federation-alert.env}"
WAIT_SEC="${WAIT_SEC:-180}"

echo "Send /start to @AEGIS_GOD_BOT in Telegram, waiting up to ${WAIT_SEC}s..."
end=$((SECONDS + WAIT_SEC))
while [[ $SECONDS -lt $end ]]; do
  cid=$(curl -sfS --max-time 15 "https://api.telegram.org/bot${BOT_TOKEN}/getUpdates?limit=30" \
    | python3 -c "
import sys, json
d = json.load(sys.stdin)
for u in reversed(d.get('result', [])):
    m = u.get('message') or u.get('edited_message') or {}
    c = m.get('chat', {}).get('id')
    if c is not None:
        print(c)
        break
" 2>/dev/null || true)
  if [[ -n "${cid:-}" ]]; then
    grep -v '^ALERT_TELEGRAM_CHAT_ID=' "$ENV_FILE" >"${ENV_FILE}.tmp" 2>/dev/null || true
    echo "ALERT_TELEGRAM_CHAT_ID=${cid}" >>"${ENV_FILE}.tmp"
    chmod 600 "${ENV_FILE}.tmp"
    mv "${ENV_FILE}.tmp" "$ENV_FILE"
    curl -sfS -X POST "https://api.telegram.org/bot${BOT_TOKEN}/sendMessage" \
      -d "chat_id=${cid}" \
      --data-urlencode "text=AEGIS Federation alerts connected. You will receive alerts on federation issues." >/dev/null
    echo "chat_id=${cid} saved to ${ENV_FILE}"
    exit 0
  fi
  sleep 5
done
echo "Timeout — no message to bot yet" >&2
exit 1
