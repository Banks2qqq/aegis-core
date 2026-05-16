#!/usr/bin/env bash
# Deploy federation pilot stack: backend (both nodes) + frontend + alert timer (primary).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
BIN_REMOTE="/opt/aegis/bin/agent-cli"
BACKEND_REMOTE="/opt/aegis/backend"
FRONTEND_WEB="/var/www/aegis/html"
AGENT_SRC=(federation.rs federation_auth.rs federation_client.rs metrics.rs config.rs server.rs main.rs)

ssh_primary() { ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" "$@"; }
scp_primary() { scp -o StrictHostKeyChecking=no "$@"; }

ssh_secondary() {
  if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${SECONDARY}" true 2>/dev/null; then
    ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" "$@"
  else
    [[ -n "${VPS_PASSWORD:-}" ]] || {
      echo "Secondary ${SECONDARY}: set VPS_PASSWORD or add SSH key" >&2
      return 1
    }
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" "$@"
  fi
}

scp_secondary() {
  if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${SECONDARY}" true 2>/dev/null; then
    scp -o StrictHostKeyChecking=no "$@"
  else
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e scp -o StrictHostKeyChecking=no "$@"
  fi
}

build_agent_on_host() {
  local runner=$1
  $runner "bash -lc 'source /root/.cargo/env 2>/dev/null || true; cd ${BACKEND_REMOTE} && cargo build --release --bin agent-cli && cp target/release/agent-cli ${BIN_REMOTE} && chmod 755 ${BIN_REMOTE}'"
}

deploy_primary() {
  echo "==> [primary] agent sources + build on VPS"
  for f in "${AGENT_SRC[@]}"; do
    scp_primary "$ROOT/backend/src/agent/$f" "${USER}@${PRIMARY}:${BACKEND_REMOTE}/src/agent/"
  done
  ssh_primary "systemctl stop aegis-agent || true"
  build_agent_on_host ssh_primary
  ssh_primary "systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent"
  tar czf - -C "$ROOT/frontend/out" . | ssh_primary \
    "rm -rf /root/frontend_staging ${FRONTEND_WEB}/* && mkdir -p /root/frontend_staging ${FRONTEND_WEB} && tar xzf - -C /root/frontend_staging && cp -a /root/frontend_staging/. ${FRONTEND_WEB}/ && chmod -R 755 ${FRONTEND_WEB}"
}

deploy_secondary() {
  echo "==> [secondary] agent sources + build on VPS"
  for f in "${AGENT_SRC[@]}"; do
    scp_secondary "$ROOT/backend/src/agent/$f" "${USER}@${SECONDARY}:${BACKEND_REMOTE}/src/agent/"
  done
  ssh_secondary "systemctl stop aegis-agent || true"
  build_agent_on_host ssh_secondary
  ssh_secondary "systemctl start aegis-agent; sleep 2; systemctl is-active aegis-agent"
  tar czf - -C "$ROOT/frontend/out" . | ssh_secondary \
    "rm -rf /root/frontend_staging ${FRONTEND_WEB}/* && mkdir -p /root/frontend_staging ${FRONTEND_WEB} && tar xzf - -C /root/frontend_staging && cp -a /root/frontend_staging/. ${FRONTEND_WEB}/ && chmod -R 755 ${FRONTEND_WEB}"
}

install_alert_primary() {
  echo "==> [primary] alert timer"
  ssh_primary "mkdir -p /opt/aegis/deploy/federation-alert /opt/aegis/deploy/smoke"
  scp_primary \
    "$ROOT/deploy/federation-alert/check-federation-alert.sh" \
    "$ROOT/deploy/federation-alert/wait-telegram-chat.sh" \
    "$ROOT/deploy/federation-alert/aegis-federation-alert.service" \
    "$ROOT/deploy/federation-alert/aegis-federation-alert.timer" \
    "${USER}@${PRIMARY}:/opt/aegis/deploy/federation-alert/"
  scp_primary \
    "$ROOT/deploy/smoke/lib.sh" \
    "$ROOT/deploy/smoke/integration-federation-prod-vps.sh" \
    "${USER}@${PRIMARY}:/opt/aegis/deploy/smoke/"
  ssh_primary bash -lc "
    chmod 755 /opt/aegis/deploy/federation-alert/check-federation-alert.sh
    cp /opt/aegis/deploy/federation-alert/aegis-federation-alert.{service,timer} /etc/systemd/system/
    systemctl daemon-reload
    systemctl enable --now aegis-federation-alert.timer
    systemctl is-active aegis-federation-alert.timer
    /opt/aegis/deploy/federation-alert/check-federation-alert.sh; echo exit:\$?
  "
}

echo "==> Frontend build (agent builds on Linux VPS — do not upload Mac binary)"
(cd "$ROOT/frontend" && npm run build)

deploy_primary
deploy_secondary || echo "WARN: secondary deploy skipped — export VPS_PASSWORD" >&2
install_alert_primary

echo "==> Enable autostart"
bash "${ROOT}/deploy/production-enable-services.sh" || true

echo "==> Smoke gate (VPS)"
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" \
  /opt/aegis/deploy/smoke/integration-federation-prod-vps.sh

echo ""
echo "=== federation-pilot-deploy complete ==="
