#!/usr/bin/env bash
# PR6.2 — Restore from backup-aegis.sh directory.
# Usage:
#   sudo ./deploy/restore-aegis.sh /path/to/backup/dir
# Env: DRY_RUN=1 — print actions only
set -euo pipefail

SRC="${1:?Usage: restore-aegis.sh BACKUP_DIR}"
[[ -d "$SRC" ]] || { echo "Not a directory: $SRC"; exit 1; }

CONFIG="${CONFIG:-/opt/aegis/backend/config.yaml}"

stop_svc() {
  if command -v systemctl >/dev/null 2>&1 && systemctl cat aegis-agent >/dev/null 2>&1; then
    if [[ "${DRY_RUN:-0}" == "1" ]]; then
      echo "[restore] DRY_RUN: systemctl stop aegis-agent"
    else
      systemctl stop aegis-agent || true
    fi
  fi
}

start_svc() {
  if command -v systemctl >/dev/null 2>&1 && systemctl cat aegis-agent >/dev/null 2>&1; then
    if [[ "${DRY_RUN:-0}" == "1" ]]; then
      echo "[restore] DRY_RUN: systemctl start aegis-agent"
    else
      systemctl start aegis-agent
    fi
  fi
}

manifest="$SRC/manifest.json"
SQLITE=""
DNA=""
AUDIT=""
if [[ -f "$manifest" ]]; then
  SQLITE="$(python3 -c "import json; print(json.load(open('$manifest')).get('sqlite',''))" 2>/dev/null || true)"
  DNA="$(python3 -c "import json; print(json.load(open('$manifest')).get('dna',''))" 2>/dev/null || true)"
  AUDIT="$(python3 -c "import json; print(json.load(open('$manifest')).get('audit',''))" 2>/dev/null || true)"
fi

SQLITE="${SQLITE:-/opt/aegis/backend/data/aegis.db}"
DNA="${DNA:-/opt/aegis/backend/data/aegis_dna.json}"
AUDIT="${AUDIT:-/opt/aegis/backend/data/audit.log}"

restore_file() {
  local name=$1 target=$2
  local from="$SRC/$name"
  [[ -f "$from" ]] || { echo "[restore] skip missing $name"; return 0; }
  mkdir -p "$(dirname "$target")"
  if [[ "${DRY_RUN:-0}" == "1" ]]; then
    echo "[restore] DRY_RUN cp $from → $target"
  else
    cp -a "$from" "$target"
    echo "[restore] restored $target"
  fi
}

stop_svc

restore_file "$(basename "$SQLITE")" "$SQLITE"
restore_file "$(basename "$DNA")" "$DNA"
restore_file "$(basename "$AUDIT")" "$AUDIT"
restore_file "$(basename "$CONFIG")" "$CONFIG"

start_svc
echo "[restore] Finished"
