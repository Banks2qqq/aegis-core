#!/usr/bin/env bash
# Append TI keys to /etc/aegis/agent.env on both VPS nodes.
# Keys file (gitignored): deploy/.scout-ti-keys.env
#   ABUSECH_API_KEY=...   # https://auth.abuse.ch/ → profile → Auth-Key
#   OTX_API_KEY=...
#   VT_API_KEY=...
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KEYS_FILE="${SCOUT_TI_KEYS_FILE:-${ROOT}/deploy/.scout-ti-keys.env}"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
ENV_REMOTE="/etc/aegis/agent.env"

[[ -f "$KEYS_FILE" ]] || {
  echo "Create $KEYS_FILE with:" >&2
  echo "  ABUSECH_API_KEY=your_auth_key  # https://auth.abuse.ch/" >&2
  echo "  OTX_API_KEY=your_otx_key" >&2
  echo "  VT_API_KEY=your_vt_key" >&2
  exit 1
}

apply_on() {
  local host=$1
  echo "==> [$host] merge TI keys into agent.env"
  scp -o StrictHostKeyChecking=no "$KEYS_FILE" "${USER}@${host}:/tmp/scout-ti-keys.env"
  ssh -o StrictHostKeyChecking=no "${USER}@${host}" bash -s <<'REMOTE'
set -euo pipefail
ENV=/etc/aegis/agent.env
TMP=/tmp/scout-ti-keys.env
touch "$ENV"
chmod 600 "$ENV"
for var in ABUSECH_API_KEY OTX_API_KEY VT_API_KEY; do
  sed -i "/^${var}=/d" "$ENV"
done
while IFS= read -r line || [[ -n "$line" ]]; do
  [[ "$line" =~ ^(ABUSECH_API_KEY|OTX_API_KEY|VT_API_KEY)= ]] || continue
  echo "$line" >>"$ENV"
done <"$TMP"
rm -f "$TMP"
grep -E '^(ABUSECH_API_KEY|OTX_API_KEY|VT_API_KEY)=' "$ENV" | awk -F= '{print $1"=set ("length($2)" chars)"}'
systemctl restart aegis-agent
sleep 2
systemctl is-active aegis-agent
REMOTE
}

apply_on "$PRIMARY"
apply_on "$SECONDARY" || echo "WARN: secondary skipped" >&2
echo "=== scout-apply-ti-keys complete ==="
