#!/usr/bin/env bash
# Federation resilience / chaos suite (production two-node cluster).
#
# Usage:
#   export CHAOS_CONFIRM=1
#   export VPS_PASSWORD='...'   # if no SSH key on secondary
#   export FEDERATION_SHARED_SECRET='...'
#   ./deploy/federation-chaos/run-chaos-suite.sh
#
# Env:
#   CHAOS_SCENARIOS — comma list (default: all)
#     secondary_down,primary_down,network_partition,cert_break,long_downtime,recovery
#   CHAOS_DOWNTIME_SEC — long downtime duration (default 90)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib-chaos.sh
source "${ROOT}/lib-chaos.sh"

require_chaos_confirm
require_tools

SCENARIOS="${CHAOS_SCENARIOS:-secondary_down,primary_down,network_partition,cert_break,long_downtime,recovery}"
TOK=$(fed_jwt)
PASS=0
FAIL=0

want_scenario() {
  [[ ",${SCENARIOS}," == *",$1,"* ]]
}

run_scenario() {
  local name=$1
  shift
  chaos_log "=== Scenario: ${name} ==="
  if "$@"; then
    chaos_pass "$name"
    record_result "$name" "pass"
    PASS=$((PASS + 1))
  else
    chaos_fail "$name"
    record_result "$name" "fail" "$*"
    FAIL=$((FAIL + 1))
  fi
}

# --- 1. Secondary stopped ---
scenario_secondary_down() {
  ssh_secondary 'systemctl stop aegis-agent' || true
  sleep 8
  local h
  h=$(fed_health_json "$TOK")
  local online ready
  online=$(peer_field "$h" "aegis-prod-secondary" "online")
  ready=$(peer_field "$h" "aegis-prod-secondary" "federation_ready")
  chaos_log "secondary down: online=${online} federation_ready=${ready}"
  ssh_secondary 'systemctl start aegis-agent'
  wait_federation_ready "$TOK" "aegis-prod-secondary" "$CHAOS_RECOVERY_TIMEOUT_SEC" || return 1
  [[ "$online" == "False" || "$online" == "false" || "$ready" == "False" || "$ready" == "false" ]]
}

# --- 2. Primary stopped (check from API still on primary host — expect 502; check secondary logs separately) ---
scenario_primary_down() {
  ssh_primary 'systemctl stop aegis-agent' || true
  sleep 5
  local code
  code=$(curl -sk -o /dev/null -w "%{http_code}" "${PRIMARY_URL}/health" || echo "000")
  chaos_log "primary down: /health HTTP ${code}"
  ssh_primary 'systemctl start aegis-agent'
  local i=0
  while [[ $i -lt 120 ]]; do
  code=$(curl -sk -o /dev/null -w "%{http_code}" "${PRIMARY_URL}/health" || echo "000")
    [[ "$code" == "200" ]] && break
    sleep 2
    i=$((i + 2))
  done
  [[ "$code" == "200" ]]
}

# --- 3. Block :8443 on secondary (network partition) ---
scenario_network_partition() {
  ssh_secondary 'iptables -C INPUT -p tcp --dport 8443 -j DROP 2>/dev/null || iptables -I INPUT -p tcp --dport 8443 -j DROP'
  sleep 8
  local h ready
  h=$(fed_health_json "$TOK" 2>/dev/null || echo '{"report":{"peers":[]}}')
  ready=$(peer_field "$h" "aegis-prod-secondary" "federation_ready")
  chaos_log "partition: federation_ready=${ready}"
  ssh_secondary 'iptables -D INPUT -p tcp --dport 8443 -j DROP 2>/dev/null || true'
  wait_federation_ready "$TOK" "aegis-prod-secondary" "$CHAOS_RECOVERY_TIMEOUT_SEC" || return 1
  [[ "$ready" == "False" || "$ready" == "false" ]]
}

# --- 4. Break client cert on primary ---
scenario_cert_break() {
  ssh_primary 'mv /etc/aegis/federation/primary.client.pem /etc/aegis/federation/primary.client.pem.chaos_bak'
  ssh_primary 'systemctl restart aegis-agent'
  sleep 10
  local sync_ok=1
  if curl -sfS -H "Authorization: Bearer ${TOK}" -H "Content-Type: application/json" \
    -X POST "${PRIMARY_URL}/api/federation/sync" \
    -d '{"peer_id":"aegis-prod-secondary"}' | python3 -c "
import sys,json
r=json.load(sys.stdin)
res=r.get('results') or [r.get('result')]
res=[x for x in res if x]
if res and res[0].get('success'):
    sys.exit(1)
"; then
    sync_ok=0
  fi
  chaos_log "cert break: sync failed as expected=${sync_ok}"
  ssh_primary 'mv /etc/aegis/federation/primary.client.pem.chaos_bak /etc/aegis/federation/primary.client.pem'
  ssh_primary 'systemctl restart aegis-agent'
  sleep 8
  wait_federation_ready "$TOK" "aegis-prod-secondary" "$CHAOS_RECOVERY_TIMEOUT_SEC" || return 1
  [[ "$sync_ok" -eq 0 ]]
}

# --- 5. Long downtime ---
scenario_long_downtime() {
  ssh_secondary 'systemctl stop aegis-agent' || true
  chaos_log "long downtime ${CHAOS_DOWNTIME_SEC}s..."
  sleep "$CHAOS_DOWNTIME_SEC"
  local h
  h=$(fed_health_json "$TOK")
  local st
  st=$(peer_field "$h" "aegis-prod-secondary" "status")
  chaos_log "after downtime status=${st}"
  local t0=$SECONDS
  ssh_secondary 'systemctl start aegis-agent'
  wait_federation_ready "$TOK" "aegis-prod-secondary" "$CHAOS_RECOVERY_TIMEOUT_SEC" || return 1
  local recovery=$((SECONDS - t0))
  chaos_log "recovery took ${recovery}s"
  record_result "recovery_seconds" "info" "$recovery"
  [[ "$st" == "offline" || "$st" == "degraded" || "$st" == "" ]]
}

# --- 6. Measure recovery after brief stop ---
scenario_recovery() {
  ssh_secondary 'systemctl stop aegis-agent' || true
  sleep 5
  local t0=$SECONDS
  ssh_secondary 'systemctl start aegis-agent'
  wait_federation_ready "$TOK" "aegis-prod-secondary" "$CHAOS_RECOVERY_TIMEOUT_SEC" || return 1
  local sec=$((SECONDS - t0))
  chaos_log "recovery time: ${sec}s"
  record_result "recovery_brief_seconds" "info" "$sec"
  [[ "$sec" -lt "$CHAOS_RECOVERY_TIMEOUT_SEC" ]]
}

chaos_log "Results → ${RESULTS_DIR}/chaos-run.jsonl"
: > "${RESULTS_DIR}/chaos-run.jsonl"

want_scenario secondary_down && run_scenario secondary_down scenario_secondary_down
want_scenario primary_down && run_scenario primary_down scenario_primary_down
want_scenario network_partition && run_scenario network_partition scenario_network_partition
want_scenario cert_break && run_scenario cert_break scenario_cert_break
want_scenario long_downtime && run_scenario long_downtime scenario_long_downtime
want_scenario recovery && run_scenario recovery scenario_recovery

chaos_log "Done: ${PASS} passed, ${FAIL} failed"
[[ "$FAIL" -eq 0 ]]
