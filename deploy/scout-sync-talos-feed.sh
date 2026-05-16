#!/usr/bin/env bash
# Sync Talos IP blocklist to /opt/aegis/feeds/talos-ip-blacklist.txt on VPS nodes.
set -euo pipefail
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
FEED_REMOTE="/opt/aegis/feeds/talos-ip-blacklist.txt"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOCAL_BOOT="${ROOT}/deploy/feeds/talos-ip-blacklist.txt"

URLS=(
  "${TALOS_BLOCKLIST_URL:-}"
  "https://www.talosintelligence.com/documents/ip-blacklist"
  "https://talosintelligence.com/feeds/ip_filter.csv"
)

fetch_feed() {
  local out=$1
  for u in "${URLS[@]}"; do
    [[ -n "$u" ]] || continue
    if curl -fsSL --max-time 45 -A "AEGIS-Scout/2.0" "$u" -o "$out" 2>/dev/null; then
      if grep -qE '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' "$out" 2>/dev/null; then
        echo "  fetched: $u"
        return 0
      fi
    fi
  done
  return 1
}

sync_host() {
  local host=$1
  echo "==> [$host] talos feed"
  local tmp
  tmp="$(mktemp)"
  if fetch_feed "$tmp"; then
    scp -o StrictHostKeyChecking=no "$tmp" "root@${host}:${FEED_REMOTE}"
  else
    echo "  official feed unavailable — installing bootstrap TEST-NET list"
    ssh -o StrictHostKeyChecking=no "root@${host}" "mkdir -p /opt/aegis/feeds"
    scp -o StrictHostKeyChecking=no "$LOCAL_BOOT" "root@${host}:${FEED_REMOTE}"
  fi
  rm -f "$tmp"
  ssh -o StrictHostKeyChecking=no "root@${host}" "wc -l ${FEED_REMOTE}; head -3 ${FEED_REMOTE}"
}

sync_host "$PRIMARY"
sync_host "$SECONDARY"
echo "=== scout-sync-talos-feed complete ==="
