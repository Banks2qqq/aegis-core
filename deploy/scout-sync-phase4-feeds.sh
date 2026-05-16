#!/usr/bin/env bash
# Phase 4: mirror RU blog RSS feeds to /opt/aegis/feeds on VPS (BI.ZONE, FACCT, RT-Solar, PT).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST="${1:-${PRIMARY_HOST:-178.236.16.101}}"
USER="${VPS_USER:-root}"
FEEDS_LOCAL="${ROOT}/deploy/feeds"

echo "==> Upload bundled pilot mirrors (if present)"
ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "mkdir -p /opt/aegis/feeds"
for f in pt-analytics-rss.xml bi-zone-rss.xml facct-rss.xml rt-solar-rss.xml; do
  if [[ -f "${FEEDS_LOCAL}/${f}" ]]; then
    scp -o StrictHostKeyChecking=no "${FEEDS_LOCAL}/${f}" "${USER}@${HOST}:/opt/aegis/feeds/${f}"
    echo "  bundled → /opt/aegis/feeds/${f}"
  fi
done

ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" bash -s <<'REMOTE'
set -euo pipefail
mkdir -p /opt/aegis/feeds
UA="Mozilla/5.0 (compatible; AEGIS-Scout/2.0; +https://aegis-security.ru)"

fetch_feed() {
  local name=$1 url=$2 out=$3
  if curl -fsSL --max-time 25 -A "$UA" "$url" -o "/tmp/${name}.xml" && grep -q '<item' "/tmp/${name}.xml"; then
    mv "/tmp/${name}.xml" "$out"
    echo "OK $name → $out ($(wc -c < "$out") bytes)"
    return 0
  fi
  echo "SKIP $name ($url)"
  return 1
}

fetch_feed pt_analytics "https://www.ptsecurity.com/ru-ru/about/news/rss/" /opt/aegis/feeds/pt-analytics-rss.xml || true

# Try common WordPress / blog feed paths (site may block non-RU IPs — retry from VPS)
for url in \
  "https://bi.zone/expertise/blog/feed/" \
  "https://bi.zone/feed/"; do
  fetch_feed bi_zone "$url" /opt/aegis/feeds/bi-zone-rss.xml && break || true
done

for url in \
  "https://www.facct.ru/feed/" \
  "https://www.facct.ru/blog/feed/"; do
  fetch_feed facct "$url" /opt/aegis/feeds/facct-rss.xml && break || true
done

for url in \
  "https://rt-solar.ru/feed/" \
  "https://rt-solar.ru/blog/feed/"; do
  fetch_feed rt_solar "$url" /opt/aegis/feeds/rt-solar-rss.xml && break || true
done

ls -la /opt/aegis/feeds/
REMOTE

echo "==> phase4 feeds sync done on ${HOST}"
