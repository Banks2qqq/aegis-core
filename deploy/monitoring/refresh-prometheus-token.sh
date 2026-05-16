#!/usr/bin/env bash
# Refresh Prometheus bearer_token (JWT) for /metrics scrape.
set -euo pipefail
ENV_FILE="${AGENT_ENV:-/etc/aegis/agent.env}"
TOKEN_FILE="${TOKEN_FILE:-/etc/aegis/monitoring/bearer_token}"
[[ -r "$ENV_FILE" ]] || exit 0
# shellcheck disable=SC1091
source "$ENV_FILE"
[[ -n "${AEGIS_MONITOR_API_KEY:-}" ]] || exit 0
JWT="$(curl -sf -X POST http://127.0.0.1:8080/api/login \
  -H "Content-Type: application/json" \
  -d "{\"api_key\":\"${AEGIS_MONITOR_API_KEY}\"}" \
  | python3 -c "import sys,json; print(json.load(sys.stdin).get('access_token',''))")"
[[ -n "$JWT" ]] || exit 1
printf '%s' "$JWT" >"$TOKEN_FILE"
chmod 644 "$TOKEN_FILE"
