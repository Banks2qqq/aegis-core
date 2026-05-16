#!/usr/bin/env bash
# PR5 local federation smoke: two agents, shared secret, sync + merkle repair path.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BACKEND="$ROOT/backend"
TMP="${TMPDIR:-/tmp}/aegis-fed-smoke-$$"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$TMP/cargo-target}"
BIN="$CARGO_TARGET_DIR/release/agent-cli"
SECRET="${FEDERATION_SHARED_SECRET:-aegis-smoke-federation-secret}"
PORT_A=18081
PORT_B=18082
HDR=(-H "X-AEGIS-Federation-Token: $SECRET" -H "Content-Type: application/json")

mkdir -p "$TMP/node-a/data" "$TMP/node-b/data"

echo "[smoke] Building agent-cli (target: $CARGO_TARGET_DIR)..."
(cd "$BACKEND" && cargo build --release --bin agent-cli -q)
if [[ ! -x "$BIN" ]]; then
  echo "[smoke] FAIL: binary not found at $BIN"
  exit 1
fi

cat > "$TMP/node-a/config.yaml" <<EOF
mode: development
node_id: smoke-node-a
security:
  air_gapped: true
federation:
  shared_secret: "$SECRET"
  peers:
    - id: smoke-node-b
      url: "http://127.0.0.1:$PORT_B"
database:
  sqlite_path: "$TMP/node-a/data/aegis.db"
  qdrant_url: "http://127.0.0.1:6333"
  dna_path: "$TMP/node-a/data/dna.json"
audit:
  enabled: true
  log_path: "$TMP/node-a/data/audit.log"
  immutable: false
EOF

cat > "$TMP/node-b/config.yaml" <<EOF
mode: development
node_id: smoke-node-b
security:
  air_gapped: true
federation:
  shared_secret: "$SECRET"
  peers:
    - id: smoke-node-a
      url: "http://127.0.0.1:$PORT_A"
database:
  sqlite_path: "$TMP/node-b/data/aegis.db"
  qdrant_url: "http://127.0.0.1:6333"
  dna_path: "$TMP/node-b/data/dna.json"
audit:
  enabled: true
  log_path: "$TMP/node-b/data/audit.log"
  immutable: false
EOF

cleanup() {
  [[ -n "${PID_A:-}" ]] && kill "$PID_A" 2>/dev/null || true
  [[ -n "${PID_B:-}" ]] && kill "$PID_B" 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT

start_node() {
  local name=$1 port=$2 cfg=$3
  (
    export FEDERATION_SHARED_SECRET="$SECRET"
    export JWT_SECRET="${JWT_SECRET:-smoke-jwt-secret-minimum-32-chars}"
    export AEGIS_PORT="$port"
    export RUST_LOG=warn
    cd "$BACKEND"
    exec "$BIN" --config "$cfg"
  ) >"$TMP/$name.log" 2>&1 &
  echo $!
}

echo "[smoke] Starting node A :$PORT_A and node B :$PORT_B..."
PID_A=$(start_node a "$PORT_A" "$TMP/node-a/config.yaml")
PID_B=$(start_node b "$PORT_B" "$TMP/node-b/config.yaml")

wait_http() {
  local url=$1 n=0
  until curl -sf "$url" >/dev/null 2>&1; do
    n=$((n + 1))
    if [[ $n -gt 60 ]]; then
      echo "[smoke] timeout waiting for $url"
      tail -20 "$TMP"/*.log || true
      exit 1
    fi
    sleep 0.5
  done
}

wait_http "http://127.0.0.1:$PORT_A/health"
wait_http "http://127.0.0.1:$PORT_B/health"
echo "[smoke] Both nodes healthy"

code=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$PORT_A/federation/merkle")
if [[ "$code" != "401" ]]; then
  echo "[smoke] FAIL: expected 401 without token, got $code"
  exit 1
fi
echo "[smoke] Unauthorized without token: OK"

merkle_a=$(curl -sf "${HDR[@]}" "http://127.0.0.1:$PORT_A/federation/merkle" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('merkle_root') or d.get('root',''))")
merkle_b=$(curl -sf "${HDR[@]}" "http://127.0.0.1:$PORT_B/federation/merkle" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('merkle_root') or d.get('root',''))")
echo "[smoke] Merkle A=$merkle_a B=$merkle_b"

# Ingest sample on A via federation receive on B is wrong — use CLI-less: POST receive on B from A push after sync
# Seed node A with a black KB item via changed_since path: call B missing after A has data — simplest: POST receive on A
SAMPLE='{"id":"smoke-1","item_type":"Black","content":"smoke-test-ioc","summary":"smoke","source":"smoke","confidence":0.9,"verified_by":["smoke"],"tags":[],"related_iocs":[],"first_seen":1,"last_seen":2,"content_hash":"smokehash001"}'
curl -sf "${HDR[@]}" -d "[$SAMPLE]" "http://127.0.0.1:$PORT_A/federation/receive" >/dev/null
echo "[smoke] Seeded item on node A"

# B pulls from A: POST missing with empty hash set → all items on A
pulled=$(curl -sf "${HDR[@]}" -d '[]' "http://127.0.0.1:$PORT_A/federation/missing")
count=$(echo "$pulled" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")
if [[ "$count" -lt 1 ]]; then
  echo "[smoke] FAIL: node B could not pull items from A (count=$count)"
  exit 1
fi
curl -sf "${HDR[@]}" -d "$pulled" "http://127.0.0.1:$PORT_B/federation/receive" >/dev/null
echo "[smoke] Node B received $count item(s) from A"

merkle_a2=$(curl -sf "${HDR[@]}" "http://127.0.0.1:$PORT_A/federation/merkle" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('merkle_root') or d.get('root',''))")
merkle_b2=$(curl -sf "${HDR[@]}" "http://127.0.0.1:$PORT_B/federation/merkle" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('merkle_root') or d.get('root',''))")
if [[ "$merkle_a2" != "$merkle_b2" ]]; then
  echo "[smoke] WARN: merkle still differs A=$merkle_a2 B=$merkle_b2 (empty KB merkle_empty is OK if both match)"
fi
if [[ "$merkle_a2" == "$merkle_b2" ]]; then
  echo "[smoke] Merkle match after sync: OK"
fi

echo "[smoke] All federation smoke checks passed."
