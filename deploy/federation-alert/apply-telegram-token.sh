#!/usr/bin/env bash
# Apply NEW Telegram bot token after BotFather /revoke (never commit the token).
# Usage:
#   echo 'YOUR_NEW_TOKEN' > deploy/federation-alert/.telegram-token
#   chmod 600 deploy/federation-alert/.telegram-token
#   ./deploy/federation-alert/apply-telegram-token.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
TOKEN_FILE="${ROOT}/deploy/federation-alert/.telegram-token"

if [[ -n "${ALERT_TELEGRAM_BOT_TOKEN:-}" ]]; then
  TOKEN="$ALERT_TELEGRAM_BOT_TOKEN"
elif [[ -f "$TOKEN_FILE" ]]; then
  TOKEN="$(tr -d '[:space:]' < "$TOKEN_FILE")"
else
  echo "Put new token in ${TOKEN_FILE} or export ALERT_TELEGRAM_BOT_TOKEN" >&2
  exit 1
fi

CHAT_ID="${ALERT_TELEGRAM_CHAT_ID:-1042900815}"

echo "==> Validate token"
curl -sfS "https://api.telegram.org/bot${TOKEN}/getMe" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['ok']; print('bot:', d['result'].get('username'))"

export ALERT_TELEGRAM_BOT_TOKEN="$TOKEN"
export ALERT_TELEGRAM_CHAT_ID="$CHAT_ID"
"${ROOT}/deploy/federation-alert/setup-telegram-alert.sh"

echo "==> Sync to secondary"
scp -o StrictHostKeyChecking=no "root@${PRIMARY}:/etc/aegis/federation-alert.env" /tmp/aegis-fed-alert.env.$$
scp -o StrictHostKeyChecking=no /tmp/aegis-fed-alert.env.$$ "root@${SECONDARY}:/etc/aegis/federation-alert.env"
ssh -o StrictHostKeyChecking=no "root@${SECONDARY}" "chmod 600 /etc/aegis/federation-alert.env"
rm -f /tmp/aegis-fed-alert.env.$$

echo "==> Test from primary VPS"
ssh -o StrictHostKeyChecking=no "root@${PRIMARY}" \
  'ALERT_TEST_NOTIFY=1 /opt/aegis/deploy/federation-alert/check-federation-alert.sh'
echo "Check Telegram for test message."
