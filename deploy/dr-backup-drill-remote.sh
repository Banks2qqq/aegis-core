#!/usr/bin/env bash
# Runs ON the VPS during D1 drill (invoked by dr-backup-drill.sh).
set -euo pipefail

DRILL_DIR="${1:?usage: dr-backup-drill-remote.sh DEST_DIR}"
MAX_SEC="${DRILL_MAX_SEC:-1800}"
SQLITE=/opt/aegis/backend/data/aegis.db
DNA=/opt/aegis/backend/data/aegis_dna.json
START=$(date +%s)

kb_count() {
  sqlite3 "$SQLITE" "SELECT COUNT(*) FROM knowledge_items;" 2>/dev/null || echo 0
}

echo "[drill] 1/5 backup → $DRILL_DIR"
/opt/aegis/deploy/backup-aegis.sh "$DRILL_DIR"
[[ -f "$DRILL_DIR/manifest.json" ]] || { echo "FAIL: no manifest"; exit 1; }

COUNT=$(kb_count)
DNA_SZ=$(wc -c <"$DNA")
echo "[drill] fingerprint items=$COUNT dna_bytes=$DNA_SZ"

echo "[drill] 2/5 simulate data loss"
systemctl stop aegis-agent
mv "$SQLITE" "${SQLITE}.drill-lost"
mv "$DNA" "${DNA}.drill-lost"

echo "[drill] 3/5 restore"
/opt/aegis/deploy/restore-aegis.sh "$DRILL_DIR"
sleep 5

echo "[drill] 4/5 verify"
COUNT2=$(kb_count)
DNA_SZ2=$(wc -c <"$DNA")
[[ "$COUNT" == "$COUNT2" ]] || { echo "FAIL: knowledge_items $COUNT != $COUNT2"; exit 1; }
[[ "$DNA_SZ" == "$DNA_SZ2" ]] || { echo "FAIL: dna size $DNA_SZ != $DNA_SZ2"; exit 1; }
systemctl is-active aegis-agent >/dev/null
curl -sf http://127.0.0.1:8080/health >/dev/null || { echo "FAIL: /health"; exit 1; }

ELAPSED=$(( $(date +%s) - START ))
echo "[drill] 5/5 elapsed=${ELAPSED}s (max ${MAX_SEC}s)"
[[ "$ELAPSED" -lt "$MAX_SEC" ]] || { echo "FAIL: drill exceeded ${MAX_SEC}s"; exit 1; }

rm -f "${SQLITE}.drill-lost" "${DNA}.drill-lost"
echo "DRILL_PASS elapsed=${ELAPSED}s items=${COUNT2}"
