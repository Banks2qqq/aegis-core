#!/usr/bin/env bash
# Branch A — integration bundle (H1–H3 + auth + deception).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SMOKE="${ROOT}/deploy/smoke"
# shellcheck source=lib.sh
source "${SMOKE}/lib.sh"

BASE="${BASE_URL:-http://127.0.0.1:8080}"
[[ -f /etc/aegis/agent.env ]] && source /etc/aegis/agent.env
export SMOKE_API_KEY="${SMOKE_API_KEY:-${AEGIS_MONITOR_API_KEY:-}}"
[[ -n "$SMOKE_API_KEY" ]] || die "SMOKE_API_KEY required"

echo "=== integration-honest-10-branch-a === BASE_URL=$BASE"

for s in \
  integration-heal-sandbox-real.sh \
  integration-deception-h2.sh \
  integration-heal-hitl.sh \
  integration-auth-h5.sh \
  integration-demo-e2e.sh; do
  echo "--- $s"
  bash "${SMOKE}/${s}"
done

echo "=== integration-honest-10-branch-a: OK ==="
