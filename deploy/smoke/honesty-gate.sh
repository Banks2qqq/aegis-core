#!/usr/bin/env bash
# H8 — Honesty gate: branch A 10/10 (run on VPS or with BASE_URL + agent.env).
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SMOKE="${ROOT}/deploy/smoke"
# shellcheck source=lib.sh
source "${SMOKE}/lib.sh"

GATE_PASS=0
GATE_FAIL=0
GATE_WARN=0

gate_ok() { GATE_PASS=$((GATE_PASS + 1)); echo "[honesty-gate] OK: $*"; }
gate_fail() { GATE_FAIL=$((GATE_FAIL + 1)); echo "[honesty-gate] FAIL: $*" >&2; }
gate_warn() { GATE_WARN=$((GATE_WARN + 1)); echo "[honesty-gate] WARN: $*"; }

BASE="${BASE_URL:-http://127.0.0.1:8080}"
PUBLIC="${PUBLIC_URL:-}"
[[ -f /etc/aegis/agent.env ]] && source /etc/aegis/agent.env
export SMOKE_API_KEY="${SMOKE_API_KEY:-${AEGIS_MONITOR_API_KEY:-}}"
API_KEY="${SMOKE_API_KEY:-}"

echo "=== honesty-gate (branch A) === host=$(hostname) BASE=$BASE PUBLIC=${PUBLIC:-none}"

require_tools

# --- 1. Agent health ---
if curl -sf "${BASE}/health" | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='ok'" 2>/dev/null; then
  gate_ok "GET /health"
else
  gate_fail "GET /health"
fi

# --- 2. Prod flags (signed expectations) ---
DEV_MODE="${AEGIS_DEV_MODE:-0}"
HEAL_APPLY="${AEGIS_HEAL_APPLY:-0}"
CONTAIN_ENFORCE="${AEGIS_CONTAIN_ENFORCE:-0}"
SANDBOX_RUNTIME="${AEGIS_SANDBOX_RUNTIME:-off}"
echo "[honesty-gate] env: DEV_MODE=$DEV_MODE HEAL_APPLY=$HEAL_APPLY CONTAIN=$CONTAIN_ENFORCE SANDBOX=$SANDBOX_RUNTIME"
[[ -n "$API_KEY" ]] || gate_fail "AEGIS_MONITOR_API_KEY not set"

# --- 3. No fake instant sandbox PASS in logs ---
if command -v journalctl >/dev/null 2>&1; then
  if journalctl -u aegis-agent --since "48 hours ago" 2>/dev/null \
    | grep -qE 'duration=0\.00s.*result=PASS|SandboxExecutor:.*duration=0\.00'; then
    gate_fail "journalctl contains instant sandbox PASS (0.00s)"
  else
    gate_ok "no fake sandbox duration=0.00s in journal"
  fi
  if journalctl -u aegis-agent --since "7 days ago" 2>/dev/null \
    | grep -qE 'Honeypot spawned with Firecracker|Firecracker VM spawned'; then
    gate_fail "misleading Firecracker honeypot log strings present"
  else
    gate_ok "no misleading Firecracker honeypot logs"
  fi
else
  gate_warn "journalctl not available"
fi

# --- 4. Hashed API keys table ---
AUTH_DB="${AEGIS_AUTH_DB:-/opt/aegis/backend/aegis_auth.db}"
if [[ -f "$AUTH_DB" ]]; then
  count="$(python3 -c "
import sqlite3, sys
try:
    c = sqlite3.connect('${AUTH_DB}').execute('SELECT COUNT(*) FROM api_keys WHERE enabled=1').fetchone()[0]
    print(c)
except Exception as e:
    print(0, file=sys.stderr)
" 2>/dev/null || echo 0)"
  if [[ "${count:-0}" -ge 1 ]]; then
    gate_ok "api_keys table has ${count} enabled key(s)"
  else
    gate_fail "api_keys empty in $AUTH_DB"
  fi
else
  gate_fail "auth db missing: $AUTH_DB"
fi

# --- 5. Auth: test-key blocked in prod ---
if [[ "$DEV_MODE" == "0" || "$DEV_MODE" == "false" ]]; then
  code="$(curl -sS -o /dev/null -w "%{http_code}" -X POST "${BASE}/api/login" \
    -H "Content-Type: application/json" -d '{"api_key":"test-key-enterprise"}' || true)"
  if [[ "$code" == "401" ]]; then
    gate_ok "test-key-enterprise → 401 (prod)"
  else
    gate_fail "test-key-enterprise expected 401 got $code"
  fi
else
  gate_warn "AEGIS_DEV_MODE=$DEV_MODE — skipping test-key 401 check"
fi

# --- 6. Login (metrics checked after integration smokes) ---
TOKEN=""
if [[ -n "$API_KEY" ]]; then
  body="$(curl -sf -X POST "${BASE}/api/login" -H "Content-Type: application/json" \
    -d "{\"api_key\":\"$API_KEY\"}" 2>/dev/null || true)"
  TOKEN="$(jwt_from_login_body "$body" 2>/dev/null || true)"
  [[ -n "$TOKEN" ]] && gate_ok "monitor API key login" || gate_fail "login failed"
fi

# --- 7. Branch A integration bundle (populates sandbox/hitl/deception metrics) ---
if [[ "${SKIP_INTEGRATION:-0}" != "1" && -n "$API_KEY" ]]; then
  if bash "${SMOKE}/integration-honest-10-branch-a.sh"; then
    gate_ok "integration-honest-10-branch-a"
  else
    gate_fail "integration-honest-10-branch-a"
  fi
fi

# --- 8. Prometheus metrics (after integration) ---
if [[ -n "${TOKEN:-}" ]]; then
  metrics="$(fetch_agent_metrics "$TOKEN" "$BASE")"
  for want in \
    aegis_healing_sandbox_result_total \
    aegis_deception_listener_total \
    aegis_heal_hitl_total; do
    if echo "$metrics" | grep -q "$want"; then
      gate_ok "metric $want"
    else
      gate_fail "missing metric $want"
    fi
  done
  ok_sources="$(echo "$metrics" | grep -c 'aegis_scout_intel_source_total{.*status="ok"' || true)"
  if [[ "${ok_sources:-0}" -ge 8 ]]; then
    gate_ok "scout intel ok sources in metrics: $ok_sources"
  elif [[ "${HONESTY_RUN_SCOUT:-0}" == "1" ]]; then
    echo "[honesty-gate] running POST /api/scout (may take several minutes)..."
    scout_tmp="$(mktemp)"
    if curl -sf -m 300 -X POST "${BASE}/api/scout" \
      -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d '{}' -o "$scout_tmp"; then
      sources_ok="$(python3 -c "import json; print(json.load(open('$scout_tmp')).get('sources_ok',0))")"
      if [[ "${sources_ok:-0}" -ge 8 ]]; then
        gate_ok "scout sources_ok=$sources_ok"
      else
        gate_fail "scout sources_ok=$sources_ok (need >=8)"
      fi
    else
      gate_fail "POST /api/scout failed"
    fi
    rm -f "$scout_tmp"
  else
    gate_warn "scout ok sources in metrics=$ok_sources (set HONESTY_RUN_SCOUT=1 for live scout)"
  fi
fi

# --- 9. Federation peers ---
if [[ -n "${TOKEN:-}" ]]; then
  fed="$(curl -sf -H "Authorization: Bearer $TOKEN" "${BASE}/api/federation/health" 2>/dev/null || true)"
  if echo "$fed" | python3 -c "
import json,sys
d=json.load(sys.stdin)
r=d.get('report') or d
pc=int(r.get('peer_count',0))
po=int(r.get('peers_online',0))
assert pc>=1 and po>=1, (pc,po)
print(f'peers {po}/{pc}')
" 2>/dev/null; then
    gate_ok "federation peers online"
  else
    gate_fail "federation health / peers"
  fi
fi

# --- 10. Public nginx (login via PUBLIC_URL for /metrics) ---
if [[ -n "$PUBLIC" && -n "$API_KEY" ]]; then
  pub_body="$(curl -sf -X POST "${PUBLIC}/api/login" -H "Content-Type: application/json" \
    -d "{\"api_key\":\"$API_KEY\"}" 2>/dev/null || true)"
  pub_token="$(jwt_from_login_body "$pub_body" 2>/dev/null || true)"
  if [[ -n "$pub_token" ]] && curl -sf -H "Authorization: Bearer $pub_token" "${PUBLIC}/metrics" \
    | grep -q '^# HELP aegis_'; then
    gate_ok "HTTPS /metrics prometheus"
  else
    gate_fail "HTTPS /metrics not prometheus (deploy nginx-aegis-*-full.conf location /metrics)"
  fi
  if curl -sf "${PUBLIC}/api/status/public" | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='ok'" 2>/dev/null; then
    gate_ok "HTTPS /api/status/public"
  else
    gate_fail "/api/status/public"
  fi
fi

# --- 11. Pilot POST (local or public) ---
PILOT_URL="${PUBLIC:-$BASE}"
code="$(curl -sS -o /tmp/aegis-pilot-gate.json -w "%{http_code}" \
  -X POST "${PILOT_URL}/api/pilot" \
  -H "Content-Type: application/json" \
  -d '{"name":"Gate","company":"AEGIS","email":"gate@test","message":"H8"}' || true)"
if [[ "$code" == "200" ]]; then
  gate_ok "POST /api/pilot"
  if [[ -d /opt/aegis/backend/data/pilot_requests ]]; then
    n="$(ls -1 /opt/aegis/backend/data/pilot_requests/*.json 2>/dev/null | wc -l | tr -d ' ')"
    [[ "${n:-0}" -ge 1 ]] && gate_ok "pilot_requests on disk ($n files)" || gate_warn "pilot_requests dir empty"
  fi
else
  gate_fail "POST /api/pilot got $code"
fi

echo ""
echo "=== honesty-gate summary: PASS=$GATE_PASS FAIL=$GATE_FAIL WARN=$GATE_WARN ==="
if [[ "$GATE_FAIL" -gt 0 ]]; then
  exit 1
fi
exit 0
