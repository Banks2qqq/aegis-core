#!/usr/bin/env bash
# PR6.1 — Federation integration (delegates to PR5 local two-node smoke).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
exec bash "$ROOT/deploy/pr5-federation-smoke.sh"
