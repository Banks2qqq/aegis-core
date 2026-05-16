#!/usr/bin/env bash
# Production VPS smoke bundle (run on node or via ssh with BASE_URL set).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SMOKE="${ROOT}/deploy/smoke"
# shellcheck source=lib.sh
source "${SMOKE}/lib.sh"

if [[ -f /etc/aegis/agent.env ]]; then
  # shellcheck disable=SC1091
  source /etc/aegis/agent.env
  export SMOKE_API_KEY="${SMOKE_API_KEY:-${AEGIS_MONITOR_API_KEY:-}}"
fi

[[ -n "${BASE_URL:-}" ]] || die "set BASE_URL (e.g. https://aegis-security.ru)"

echo "=== smoke-prod-vps === BASE_URL=$BASE_URL"
export EXPECT_HEAL_APPLY="${EXPECT_HEAL_APPLY:-0}"
export EXPECT_CONTAIN_ENFORCE="${EXPECT_CONTAIN_ENFORCE:-0}"

for s in \
  integration-react-status.sh \
  integration-healing-registry.sh \
  integration-heal-apply.sh \
  integration-scout-contain.sh \
  integration-scout-stage2.sh \
  integration-raft-recovery.sh; do
  echo "--- $s"
  bash "$SMOKE/$s"
done

if [[ "${RUN_SCOUT_AUTONOMY:-0}" == "1" ]]; then
  echo "--- integration-scout-autonomy.sh (long)"
  bash "$SMOKE/integration-scout-autonomy.sh"
fi

echo "=== smoke-prod-vps: all steps passed ==="
