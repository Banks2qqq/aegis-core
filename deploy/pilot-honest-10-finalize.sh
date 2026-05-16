#!/usr/bin/env bash
# H8 — full honest 10/10 gate: both VPS + federation chaos + summary.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"

sync_smokes() {
  local host=$1
  scp -o StrictHostKeyChecking=no -r \
    "${ROOT}/deploy/smoke/lib.sh" \
    "${ROOT}/deploy/smoke/honesty-gate.sh" \
    "${ROOT}/deploy/smoke/integration-honest-10-branch-a.sh" \
    "${ROOT}/deploy/smoke/integration-pilot-landing.sh" \
    "${ROOT}/deploy/smoke/integration-auth-h5.sh" \
    "${ROOT}/deploy/smoke/integration-nginx-metrics-h6.sh" \
    "${ROOT}/deploy/smoke/integration-heal-sandbox-real.sh" \
    "${ROOT}/deploy/smoke/integration-deception-h2.sh" \
    "${ROOT}/deploy/smoke/integration-heal-hitl.sh" \
    "${ROOT}/deploy/smoke/integration-demo-e2e.sh" \
    "${ROOT}/deploy/smoke/integration-react-status.sh" \
    "${ROOT}/deploy/smoke/integration-federation-prod-vps.sh" \
    "${ROOT}/deploy/smoke/integration-scout-honest-10.sh" \
    "${ROOT}/deploy/smoke/integration-scout-phase4.sh" \
    "${ROOT}/deploy/smoke/smoke-prod-vps.sh" \
    "${USER}@${host}:/opt/aegis/deploy/smoke/"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
    "chmod +x /opt/aegis/deploy/smoke/*.sh"
}

run_gate() {
  local host=$1
  local base_public=$2
  local expect_heal=$3
  local expect_contain=$4
  echo "==> honesty-gate @ ${host}"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<REMOTE
set -uo pipefail
source /etc/aegis/agent.env
export BASE_URL=http://127.0.0.1:8080
export PUBLIC_URL=${base_public}
export SMOKE_API_KEY="\${AEGIS_MONITOR_API_KEY}"
export EXPECT_HEAL_APPLY=${expect_heal}
export EXPECT_CONTAIN_ENFORCE=${expect_contain}
export HONESTY_RUN_SCOUT=\${HONESTY_RUN_SCOUT:-0}
/opt/aegis/deploy/smoke/honesty-gate.sh
REMOTE
}

apply_nginx_metrics() {
  local host=$1 conf=$2 name=$3
  echo "==> nginx /metrics @ ${host} (${name})"
  scp -o StrictHostKeyChecking=no "${ROOT}/deploy/${conf}" \
    "${USER}@${host}:/etc/nginx/sites-available/${name}"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<REMOTE
set -euo pipefail
ln -sf "/etc/nginx/sites-available/${name}" "/etc/nginx/sites-enabled/${name}"
# Drop wrong site symlink if primary config was copied to secondary earlier.
for other in aegis aegis-node2; do
  [[ "\$other" == "${name}" ]] && continue
  rm -f "/etc/nginx/sites-enabled/\$other"
done
nginx -t && systemctl reload nginx
REMOTE
}

echo "=== pilot-honest-10-finalize ==="
apply_nginx_metrics "$PRIMARY" "nginx-aegis-full.conf" "aegis"
apply_nginx_metrics "$SECONDARY" "nginx-aegis-node2-full.conf" "aegis-node2"
sync_smokes "$PRIMARY"
sync_smokes "$SECONDARY"

run_gate "$SECONDARY" "https://node2.aegis-security.ru" 1 1
run_gate "$PRIMARY" "https://aegis-security.ru" 0 0

echo "==> federation prod smoke (primary)"
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
  "bash /opt/aegis/deploy/smoke/integration-federation-prod-vps.sh"

if [[ "${SKIP_CHAOS:-0}" != "1" ]]; then
  echo "==> federation chaos 6/6 (from Mac)"
  CHAOS_CONFIRM=1 bash "${ROOT}/deploy/federation-chaos/run-chaos-from-mac.sh"
else
  echo "==> SKIP_CHAOS=1"
fi

echo ""
echo "=== pilot-honest-10-finalize: ALL GATES PASSED ==="
