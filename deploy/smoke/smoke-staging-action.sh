#!/usr/bin/env bash
# Staging (secondary): real heal apply + contain host enforcement.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SMOKE="${ROOT}/deploy/smoke"

[[ -n "${BASE_URL:-}" ]] || export BASE_URL=https://node2.aegis-security.ru
export EXPECT_HEAL_APPLY=1
export EXPECT_CONTAIN_ENFORCE=1

echo "=== smoke-staging-action === BASE_URL=$BASE_URL"
bash "$SMOKE/integration-heal-apply.sh"
bash "$SMOKE/integration-scout-contain.sh"
echo "=== smoke-staging-action: passed ==="
