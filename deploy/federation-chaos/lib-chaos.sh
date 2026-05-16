# shellcheck shell=bash
# Shared helpers for federation chaos / resilience tests.
set -euo pipefail

CHAOS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${CHAOS_ROOT}/../.." && pwd)"
# shellcheck source=../smoke/lib.sh
source "${REPO_ROOT}/deploy/smoke/lib.sh"

PRIMARY_HOST="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY_HOST="${SECONDARY_HOST:-93.189.230.72}"
PRIMARY_URL="${PRIMARY_URL:-https://aegis-security.ru}"
SECONDARY_URL="${SECONDARY_URL:-https://node2.aegis-security.ru}"
if [[ -f /etc/aegis/agent.env ]]; then
  # shellcheck disable=SC1091
  source /etc/aegis/agent.env
fi
API_KEY="${SMOKE_API_KEY:-${AEGIS_MONITOR_API_KEY:-test-key-enterprise}}"
CHAOS_DOWNTIME_SEC="${CHAOS_DOWNTIME_SEC:-90}"
CHAOS_RECOVERY_TIMEOUT_SEC="${CHAOS_RECOVERY_TIMEOUT_SEC:-180}"
RESULTS_DIR="${RESULTS_DIR:-/tmp/aegis-chaos-results}"

mkdir -p "$RESULTS_DIR"

chaos_log() { echo "[chaos] $*"; }
chaos_pass() { echo "[chaos] PASS: $*"; }
chaos_fail() { echo "[chaos] FAIL: $*" >&2; }

require_chaos_confirm() {
  [[ "${CHAOS_CONFIRM:-}" == "1" ]] || die "Set CHAOS_CONFIRM=1 to run destructive federation chaos tests on live VPS"
}

CHAOS_SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=15)

ssh_primary() { ssh "${CHAOS_SSH_OPTS[@]}" "root@${PRIMARY_HOST}" "$@"; }

ssh_secondary() {
  if ssh -o BatchMode=yes "${CHAOS_SSH_OPTS[@]}" "root@${SECONDARY_HOST}" true 2>/dev/null; then
    ssh "${CHAOS_SSH_OPTS[@]}" "root@${SECONDARY_HOST}" "$@"
  else
    [[ -n "${VPS_PASSWORD:-}" ]] || die "Set VPS_PASSWORD for ${SECONDARY_HOST}"
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e ssh -o StrictHostKeyChecking=no "root@${SECONDARY_HOST}" "$@"
  fi
}

fed_jwt() {
  curl -sfS -X POST "${PRIMARY_URL}/api/login" \
    -H "Content-Type: application/json" \
    -d "{\"api_key\":\"${API_KEY}\"}" \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['access_token'])"
}

fed_health_json() {
  local tok=$1
  curl -sfS -H "Authorization: Bearer ${tok}" "${PRIMARY_URL}/api/federation/health"
}

fed_raft_json() {
  local tok=$1
  curl -sfS -H "Authorization: Bearer ${tok}" "${PRIMARY_URL}/api/raft/status"
}

peer_field() {
  local json=$1 peer_id=$2 field=$3
  python3 -c "
import json,sys
d=json.loads(sys.argv[1])
peers=d.get('report',{}).get('peers',[])
for p in peers:
    if p.get('id')==sys.argv[2]:
        v=p.get(sys.argv[3])
        print('' if v is None else v)
        break
" "$json" "$peer_id" "$field"
}

wait_peer_status() {
  local tok=$1 peer_id=$2 want_status=$3 timeout=${4:-60}
  local i=0
  while [[ $i -lt $timeout ]]; do
    local h
    h=$(fed_health_json "$tok")
    local st
    st=$(peer_field "$h" "$peer_id" "status")
    if [[ "$st" == "$want_status" ]]; then
      chaos_log "peer ${peer_id} status=${st} (wanted ${want_status}) after ${i}s"
      return 0
    fi
    sleep 2
    i=$((i + 2))
  done
  return 1
}

wait_federation_ready() {
  local tok=$1 peer_id=$2 timeout=${3:-120}
  local i=0
  while [[ $i -lt $timeout ]]; do
    local h ready
    h=$(fed_health_json "$tok")
    ready=$(peer_field "$h" "$peer_id" "federation_ready")
    if [[ "$ready" == "True" || "$ready" == "true" ]]; then
      chaos_log "peer ${peer_id} federation_ready after ${i}s"
      return 0
    fi
    sleep 3
    i=$((i + 3))
  done
  return 1
}

record_result() {
  local scenario=$1 result=$2 detail=${3:-}
  local ts
  ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  printf '{"ts":"%s","scenario":"%s","result":"%s","detail":"%s"}\n' \
    "$ts" "$scenario" "$result" "$detail" >> "${RESULTS_DIR}/chaos-run.jsonl"
}
