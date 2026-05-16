#!/usr/bin/env bash
# Run federation chaos from Mac (SSH to both nodes). Primary VPS cannot SSH to secondary without keys.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "${ROOT}/../.." && pwd)"

[[ "${CHAOS_CONFIRM:-}" == "1" ]] || {
  echo "export CHAOS_CONFIRM=1" >&2
  exit 1
}

# Monitor key from primary (optional override via SMOKE_API_KEY)
if [[ -z "${SMOKE_API_KEY:-}" ]]; then
  SMOKE_API_KEY="$(ssh -o StrictHostKeyChecking=no root@178.236.16.101 \
    "grep -E '^AEGIS_MONITOR_API_KEY=' /etc/aegis/agent.env | cut -d= -f2- | tr -d '\"'" 2>/dev/null || true)"
fi
export SMOKE_API_KEY

echo "Chaos from Mac → primary ${PRIMARY_HOST:-178.236.16.101} secondary ${SECONDARY_HOST:-93.189.230.72}"
exec bash "${ROOT}/run-chaos-suite.sh"
