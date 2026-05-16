# shellcheck shell=bash
# Shared helpers for smoke / integration scripts (PR6.1).

set -euo pipefail

die() {
  echo "[smoke] ERROR: $*" >&2
  exit 1
}

json_get() {
  python3 -c "import sys,json; print(json.load(sys.stdin).get('$2',''))" <<<"$1"
}

# Extract access_token from login JSON body.
jwt_from_login_body() {
  python3 -c "import sys,json; print(json.load(sys.stdin).get('access_token',''))" <<<"$1"
}

require_tools() {
  command -v curl >/dev/null 2>&1 || die "curl required"
  command -v python3 >/dev/null 2>&1 || die "python3 required"
}

http_code() {
  curl -sS -o /dev/null -w "%{http_code}" "$@"
}

# Fetch Prometheus text from agent (localhost on VPS). Nginx often serves SPA on /metrics.
fetch_agent_metrics() {
  local token=$1
  local metrics_base="${2:-http://127.0.0.1:8080}"
  local body
  body="$(curl -sf -H "Authorization: Bearer $token" "${metrics_base}/metrics" 2>/dev/null || true)"
  if echo "$body" | grep -q '^# TYPE aegis_'; then
    printf '%s' "$body"
    return 0
  fi
  local host="${METRICS_SSH_HOST:-${PRIMARY_HOST:-178.236.16.101}}"
  local user="${METRICS_SSH_USER:-root}"
  ssh -o StrictHostKeyChecking=no -o ConnectTimeout=12 "${user}@${host}" \
    "curl -sf -H 'Authorization: Bearer ${token}' http://127.0.0.1:8080/metrics" 2>/dev/null || true
}
