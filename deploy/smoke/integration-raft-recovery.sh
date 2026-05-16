#!/usr/bin/env bash
# Raft auto-recovery: after simulated all-stale, maintain_cluster should re-elect within 90s.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=lib.sh
source "${ROOT}/deploy/smoke/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
METRICS_BASE="${METRICS_URL:-http://127.0.0.1:8080}"
API_KEY="${SMOKE_API_KEY:-}"
[[ -f /etc/aegis/agent.env ]] && API_KEY="${API_KEY:-$(grep '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2-)}"
[[ -n "$API_KEY" ]] || die "set SMOKE_API_KEY"

require_tools
body="$(curl -sf -X POST "$BASE/api/login" -H "Content-Type: application/json" -d "{\"api_key\":\"$API_KEY\"}")"
TOKEN="$(jwt_from_login_body "$body")"
auth=( -H "Authorization: Bearer $TOKEN" )

raft_status_line() {
  curl -sf "${auth[@]}" "$METRICS_BASE/api/raft/status" | python3 -c "
import sys,json
d=json.load(sys.stdin)
leader=d.get('leader_id') or ''
live=sum(1 for n in d.get('nodes',[]) if n.get('status')=='live')
print(f'{leader}\t{live}')
"
}

echo "[raft-recovery] current leader"
read -r leader live < <(raft_status_line 2>/dev/null || echo -e '\t0')
[[ -n "${leader:-}" ]] || echo "  (no leader yet — waiting for maintain_cluster)"

echo "[raft-recovery] waiting up to 90s for live leader"
for i in $(seq 1 18); do
  read -r leader live < <(raft_status_line 2>/dev/null || echo -e '\t0')
  live="${live:-0}"
  if [[ -n "${leader:-}" && "${live}" -ge 1 ]]; then
    echo "  leader=${leader} live_nodes=${live} ($((i * 5))s)"
    echo "[integration-raft-recovery] OK"
    exit 0
  fi
  sleep 5
done

die "raft did not recover leader within 90s"
