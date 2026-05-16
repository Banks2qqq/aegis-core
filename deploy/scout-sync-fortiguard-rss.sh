#!/usr/bin/env bash
# Mirror FortiGuard Outbreak RSS to nodes (secondary may not reach fortiguard.com over TLS).
set -euo pipefail
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
REMOTE="/opt/aegis/feeds/fortiguard-outbreak.xml"
URL="${FORTIGUARD_RSS_URL:-https://www.fortiguard.com/rss/outbreakalert.xml}"
TMP="$(mktemp)"

echo "==> fetch $URL"
if ! curl -fsSL --max-time 45 -A "AEGIS-Scout/2.0" "$URL" -o "$TMP" 2>/dev/null; then
  echo "  local curl failed — fetch via primary VPS"
  ssh -o StrictHostKeyChecking=no "root@${PRIMARY}" \
    "curl -fsSL --max-time 45 -A 'AEGIS-Scout/2.0' '$URL' -o /tmp/fg-rss.xml && cat /tmp/fg-rss.xml" >"$TMP"
fi
[[ -s "$TMP" ]] || { echo "empty RSS" >&2; exit 1; }

for host in "$PRIMARY" "$SECONDARY"; do
  echo "==> [$host]"
  ssh -o StrictHostKeyChecking=no "root@${host}" "mkdir -p /opt/aegis/feeds"
  scp -o StrictHostKeyChecking=no "$TMP" "root@${host}:${REMOTE}"
  ssh -o StrictHostKeyChecking=no "root@${host}" "wc -c ${REMOTE}"
done
rm -f "$TMP"
echo "=== scout-sync-fortiguard-rss complete ==="
