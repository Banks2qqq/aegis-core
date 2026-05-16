#!/usr/bin/env bash
# D1 — Backup + restore drill on staging (secondary). Target: complete in <30 min.
#
# Usage:
#   ./deploy/dr-backup-drill.sh
# Env:
#   DRILL_HOST — default 93.189.230.72 (staging)
#   SKIP_SMOKE=1 — skip post-restore smoke
set -euo pipefail

DRILL_HOST="${DRILL_HOST:-93.189.230.72}"
DRILL_DIR="/root/aegis-backups/drill-$(date +%Y%m%d-%H%M%S)"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== D1 backup/restore drill === host=$DRILL_HOST dest=$DRILL_DIR"

ssh -o StrictHostKeyChecking=no "root@${DRILL_HOST}" 'mkdir -p /opt/aegis/deploy/smoke'
scp -o StrictHostKeyChecking=no \
  "$ROOT/deploy/backup-aegis.sh" \
  "$ROOT/deploy/restore-aegis.sh" \
  "$ROOT/deploy/dr-backup-drill-remote.sh" \
  "root@${DRILL_HOST}:/opt/aegis/deploy/"
scp -o StrictHostKeyChecking=no \
  "$ROOT/deploy/smoke/integration-dr-backup.sh" \
  "root@${DRILL_HOST}:/opt/aegis/deploy/smoke/"
ssh -o StrictHostKeyChecking=no "root@${DRILL_HOST}" \
  'chmod +x /opt/aegis/deploy/backup-aegis.sh /opt/aegis/deploy/restore-aegis.sh /opt/aegis/deploy/dr-backup-drill-remote.sh /opt/aegis/deploy/smoke/integration-dr-backup.sh'

RESULT="$(ssh -o StrictHostKeyChecking=no "root@${DRILL_HOST}" \
  "/opt/aegis/deploy/dr-backup-drill-remote.sh ${DRILL_DIR}" 2>&1)" || true

echo "$RESULT"
if echo "$RESULT" | grep -q 'DRILL_PASS'; then
  echo "=== D1 backup/restore drill: PASS ==="
else
  echo "=== D1 backup/restore drill: FAIL ===" >&2
  exit 1
fi

if [[ "${SKIP_SMOKE:-0}" != "1" ]]; then
  echo "==> post-restore smoke"
  ssh -o StrictHostKeyChecking=no "root@${DRILL_HOST}" \
    'export BASE_URL=https://node2.aegis-security.ru; source /etc/aegis/agent.env; export SMOKE_API_KEY="$AEGIS_MONITOR_API_KEY"; bash /opt/aegis/deploy/smoke/integration-dr-backup.sh'
fi
