#!/usr/bin/env bash
# Mirror safe-surf.ru RSS (optional — nodes usually reach it directly).
set -euo pipefail
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
REMOTE="/opt/aegis/feeds/safe-surf-rss.xml"
URL="${SAFE_SURF_RSS_URL:-https://safe-surf.ru/rss}"
TMP="$(mktemp)"

curl -fsSL --max-time 45 -A "AEGIS-Scout/2.0" "$URL" -o "$TMP"
[[ -s "$TMP" ]] || { echo "empty RSS" >&2; exit 1; }

for host in "$PRIMARY" "$SECONDARY"; do
  ssh -o StrictHostKeyChecking=no "root@${host}" "mkdir -p /opt/aegis/feeds"
  scp -o StrictHostKeyChecking=no "$TMP" "root@${host}:${REMOTE}"
done
rm -f "$TMP"
echo "=== scout-sync-safe-surf-rss complete ==="
