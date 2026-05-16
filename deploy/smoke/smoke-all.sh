#!/usr/bin/env bash
# PR6.1 — Run full local smoke + integration bundle (no VPS).
# Requires: agent-cli running on BASE_URL (default http://127.0.0.1:8080).
# Federation block starts two temporary nodes (ports 18081–18082); no conflict if API on 8080.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "=== PR6.1 smoke-all === BASE_URL=${BASE_URL:-http://127.0.0.1:8080}"

bash "$ROOT/deploy/smoke/smoke-api.sh"

bash "$ROOT/deploy/smoke/integration-healing-registry.sh"

bash "$ROOT/deploy/smoke/integration-react-status.sh"

bash "$ROOT/deploy/smoke/integration-scout-contain.sh"
bash "$ROOT/deploy/smoke/integration-scout-autonomy.sh"
bash "$ROOT/deploy/smoke/integration-scout-stage2.sh"

if [[ "${SKIP_FEDERATION:-0}" != "1" ]]; then
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/backend/target}"
  bash "$ROOT/deploy/smoke/integration-federation.sh"
else
  echo "[smoke-all] SKIP_FEDERATION=1 — two-node federation smoke skipped"
fi

echo "=== smoke-all: all steps passed ==="
