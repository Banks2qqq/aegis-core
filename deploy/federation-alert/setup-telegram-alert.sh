#!/usr/bin/env bash
# Configure Telegram alerts on primary VPS.
# Usage (from Mac):
#   export ALERT_TELEGRAM_BOT_TOKEN='...'
#   export ALERT_TELEGRAM_CHAT_ID='...'   # optional if bot already has messages
#   ./deploy/federation-alert/setup-telegram-alert.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
USER="${VPS_USER:-root}"

[[ -n "${ALERT_TELEGRAM_BOT_TOKEN:-}" ]] || {
  echo "Set ALERT_TELEGRAM_BOT_TOKEN" >&2
  exit 1
}

CHAT_ID="${ALERT_TELEGRAM_CHAT_ID:-}"
if [[ -z "$CHAT_ID" ]]; then
  echo "==> Resolving chat_id from getUpdates (message the bot first if empty)"
  CHAT_ID=$(curl -sfS "https://api.telegram.org/bot${ALERT_TELEGRAM_BOT_TOKEN}/getUpdates" \
    | python3 -c "
import sys, json
d = json.load(sys.stdin)
for u in reversed(d.get('result', [])):
    m = u.get('message') or u.get('channel_post') or {}
    cid = m.get('chat', {}).get('id')
    if cid:
        print(cid)
        break
" 2>/dev/null || true)
fi

[[ -n "$CHAT_ID" ]] || {
  echo "No chat_id. Open Telegram, send /start to your bot, re-run." >&2
  exit 1
}

echo "==> Using chat_id=${CHAT_ID}"

ENV_BODY=$(cat <<EOF
PRIMARY_URL=https://aegis-security.ru
API_KEY=${API_KEY:-test-key-enterprise}
ALERT_SYNC_LATENCY_MS=${ALERT_SYNC_LATENCY_MS:-30000}
ALERT_TELEGRAM_BOT_TOKEN=${ALERT_TELEGRAM_BOT_TOKEN}
ALERT_TELEGRAM_CHAT_ID=${CHAT_ID}
EOF
)

ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" "cat > /etc/aegis/federation-alert.env && chmod 600 /etc/aegis/federation-alert.env" <<<"$ENV_BODY"

# Deploy alert scripts
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" "mkdir -p /opt/aegis/deploy/federation-alert /opt/aegis/deploy/smoke"
scp -o StrictHostKeyChecking=no \
  "$ROOT/deploy/federation-alert/check-federation-alert.sh" \
  "$ROOT/deploy/federation-alert/aegis-federation-alert.service" \
  "$ROOT/deploy/federation-alert/aegis-federation-alert.timer" \
  "${USER}@${PRIMARY}:/opt/aegis/deploy/federation-alert/"
scp -o StrictHostKeyChecking=no "$ROOT/deploy/smoke/lib.sh" "${USER}@${PRIMARY}:/opt/aegis/deploy/smoke/"

ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" bash -lc "
  chmod 755 /opt/aegis/deploy/federation-alert/check-federation-alert.sh
  cp /opt/aegis/deploy/federation-alert/aegis-federation-alert.{service,timer} /etc/systemd/system/
  systemctl daemon-reload
  systemctl enable --now aegis-federation-alert.timer
"

if [[ -n "$CHAT_ID" ]]; then
  echo "==> Test notification"
  curl -sfS -X POST "https://api.telegram.org/bot${ALERT_TELEGRAM_BOT_TOKEN}/sendMessage" \
    -d "chat_id=${CHAT_ID}" \
    --data-urlencode "text=AEGIS Federation alerts active. Timer every 5 min."
else
  echo "==> Waiting for /start on @AEGIS_GOD_BOT (VPS, 3 min)"
  ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
    "ALERT_TELEGRAM_BOT_TOKEN='${ALERT_TELEGRAM_BOT_TOKEN}' WAIT_SEC=180 /opt/aegis/deploy/federation-alert/wait-telegram-chat.sh" || true
fi

ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
  'ALERT_TEST_NOTIFY=1 /opt/aegis/deploy/federation-alert/check-federation-alert.sh; echo check_exit:$?'

echo "Done. Logs: ssh ${USER}@${PRIMARY} journalctl -t aegis-federation-alert -n 20"
