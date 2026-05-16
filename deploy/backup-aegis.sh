#!/usr/bin/env bash
# PR6.2 — Backup SQLite KB, DNA JSON, audit log, config.
# Usage:
#   sudo ./deploy/backup-aegis.sh [DEST_DIR]
# Env:
#   CONFIG=/opt/aegis/backend/config.yaml
#   READ_AGENT_ENV=1 — also copy /etc/aegis/agent.env (secrets)
set -euo pipefail

DEST="${1:-}"
if [[ -z "$DEST" ]]; then
  DEST="./backups/aegis-$(date +%Y%m%d-%H%M%S)"
fi

CONFIG="${CONFIG:-/opt/aegis/backend/config.yaml}"

yaml_val() {
  local key="$1"
  grep -E "^[[:space:]]*${key}:" "$CONFIG" | head -1 | sed -E 's/^[^:]*:[[:space:]]*//; s/^"//; s/"$//; s/^'"'"'//; s/'"'"'$//'
}

if [[ -f "$CONFIG" ]]; then
  SQLITE="$(yaml_val sqlite_path)"
  DNA="$(yaml_val dna_path)"
  AUDIT="$(yaml_val log_path)"
else
  SQLITE=""
  DNA=""
  AUDIT=""
fi

SQLITE="${SQLITE:-/opt/aegis/backend/data/aegis.db}"
DNA="${DNA:-/opt/aegis/backend/data/aegis_dna.json}"
AUDIT="${AUDIT:-/opt/aegis/backend/data/audit.log}"

AGENT_ENV="${AGENT_ENV:-/etc/aegis/agent.env}"

mkdir -p "$DEST"

copy_if() {
  local src=$1
  [[ -e "$src" ]] || { echo "[backup] skip missing $src"; return 0; }
  cp -a "$src" "$DEST/"
  echo "[backup] stored $(basename "$src")"
}

copy_if "$SQLITE"
copy_if "$DNA"
copy_if "$AUDIT"
copy_if "$CONFIG"

if [[ "${READ_AGENT_ENV:-0}" == "1" ]] && [[ -r "$AGENT_ENV" ]]; then
  cp -a "$AGENT_ENV" "$DEST/agent.env"
  echo "[backup] stored agent.env (secrets — chmod 600 destination tree)"
fi

{
  echo '{'
  echo "  \"backup_version\": 1,"
  echo "  \"created\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
  echo "  \"config\": \"$CONFIG\","
  echo "  \"sqlite\": \"$SQLITE\","
  echo "  \"dna\": \"$DNA\","
  echo "  \"audit\": \"$AUDIT\""
  echo '}'
} >"$DEST/manifest.json"

echo "[backup] Done → $DEST"
