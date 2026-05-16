#!/usr/bin/env bash
# Production federation mTLS: distribute CA/certs, nginx ssl_verify_client, remove INSECURE_TLS.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIMARY="${PRIMARY_HOST:-178.236.16.101}"
SECONDARY="${SECONDARY_HOST:-93.189.230.72}"
USER="${VPS_USER:-root}"
CERTS="${ROOT}/deploy/federation-certs"

[[ -f "$CERTS/ca.pem" ]] || {
  echo "Run: ./deploy/generate-federation-mtls.sh" >&2
  exit 1
}

echo "==> Distribute federation CA + client certs"
PRIMARY_HOST="$PRIMARY" SECONDARY_HOST="$SECONDARY" VPS_USER="$USER" \
  "${ROOT}/deploy/distribute-federation-mtls.sh"

ssh_secondary() {
  if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${SECONDARY}" true 2>/dev/null; then
    ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" "$@"
  else
    [[ -n "${VPS_PASSWORD:-}" ]] || { echo "Set VPS_PASSWORD for ${SECONDARY}" >&2; exit 1; }
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e ssh -o StrictHostKeyChecking=no "${USER}@${SECONDARY}" "$@"
  fi
}

scp_secondary() {
  local src=$1 dest=$2
  if ssh -o BatchMode=yes -o ConnectTimeout=5 "${USER}@${SECONDARY}" true 2>/dev/null; then
    scp -o StrictHostKeyChecking=no "$src" "${USER}@${SECONDARY}:${dest}"
  else
    export SSHPASS="$VPS_PASSWORD"
    sshpass -e scp -o StrictHostKeyChecking=no "$src" "${USER}@${SECONDARY}:${dest}"
  fi
}

strip_insecure_tls() {
  local host=$1
  local run
  run='sed -i "/^AEGIS_FEDERATION_INSECURE_TLS=/d" /etc/aegis/agent.env
grep -q "^AEGIS_FEDERATION_CA_CERT=" /etc/aegis/agent.env || echo "AEGIS_FEDERATION_CA_CERT=/etc/aegis/federation/ca.pem" >>/etc/aegis/agent.env'
  if [[ "$host" == "$SECONDARY" ]]; then
    ssh_secondary "$run"
  else
    ssh -o StrictHostKeyChecking=no "${USER}@${host}" "$run"
  fi
}

echo "==> Nginx federation mTLS access log format"
for host in "$PRIMARY" "$SECONDARY"; do
  if [[ "$host" == "$SECONDARY" ]]; then
    scp_secondary "${ROOT}/deploy/nginx-federation-mtls-log.conf" /etc/nginx/conf.d/aegis-federation-mtls-log.conf
    ssh_secondary 'nginx -t && systemctl reload nginx'
  else
    scp -o StrictHostKeyChecking=no "${ROOT}/deploy/nginx-federation-mtls-log.conf" \
      "${USER}@${host}:/etc/nginx/conf.d/aegis-federation-mtls-log.conf"
    ssh -o StrictHostKeyChecking=no "${USER}@${host}" 'nginx -t && systemctl reload nginx'
  fi
done

echo "==> Firewall: allow 8443 (federation mTLS)"
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" 'ufw allow 8443/tcp comment "AEGIS federation mTLS" || true'
ssh_secondary 'ufw allow 8443/tcp comment "AEGIS federation mTLS" || true'

echo "==> Nginx: primary (443 public + 8443 federation mTLS)"
scp -o StrictHostKeyChecking=no "${ROOT}/deploy/nginx-aegis-full.conf" \
  "${USER}@${PRIMARY}:/etc/nginx/sites-available/aegis"
ssh -o StrictHostKeyChecking=no "${USER}@${PRIMARY}" 'nginx -t && systemctl reload nginx'

echo "==> Nginx: secondary (node2)"
scp_secondary "${ROOT}/deploy/nginx-aegis-node2-full.conf" /etc/nginx/sites-available/aegis
ssh_secondary 'nginx -t && systemctl reload nginx'

echo "==> agent.env: remove AEGIS_FEDERATION_INSECURE_TLS"
strip_insecure_tls "$PRIMARY"
strip_insecure_tls "$SECONDARY"

echo "==> Agent configs + federation sources"
scp -o StrictHostKeyChecking=no "${ROOT}/deploy/config.primary-with-peer.yaml" \
  "${USER}@${PRIMARY}:/opt/aegis/backend/config.yaml"
scp_secondary "${ROOT}/deploy/config.secondary.production.yaml" /opt/aegis/backend/config.yaml

for host in "$PRIMARY" "$SECONDARY"; do
  if [[ "$host" == "$SECONDARY" ]]; then
    for f in federation_client.rs federation.rs config.rs; do
      scp_secondary "${ROOT}/backend/src/agent/${f}" "/opt/aegis/backend/src/agent/${f}"
    done
    scp_secondary "${ROOT}/backend/Cargo.toml" /opt/aegis/backend/Cargo.toml
    ssh_secondary 'bash -lc "source /root/.cargo/env && cd /opt/aegis/backend && cargo build --release --bin agent-cli 2>&1 | tail -1"'
    ssh_secondary 'systemctl stop aegis-agent && cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli && systemctl start aegis-agent'
  else
    for f in federation_client.rs federation.rs config.rs; do
      scp -o StrictHostKeyChecking=no "${ROOT}/backend/src/agent/${f}" \
        "${USER}@${host}:/opt/aegis/backend/src/agent/${f}"
    done
    scp -o StrictHostKeyChecking=no "${ROOT}/backend/Cargo.toml" "${USER}@${host}:/opt/aegis/backend/Cargo.toml"
    ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
      'bash -lc "source /root/.cargo/env && cd /opt/aegis/backend && cargo build --release --bin agent-cli 2>&1 | tail -1"'
    ssh -o StrictHostKeyChecking=no "${USER}@${host}" \
      'systemctl stop aegis-agent && cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli && systemctl start aegis-agent'
  fi
done

sleep 3
echo "==> Verify"
curl -sfS "https://aegis-security.ru/health" | head -c 80; echo
curl -sfS "https://node2.aegis-security.ru/health" | head -c 80; echo
echo "Without client cert :8443/federation/merkle should fail:"
curl -s -o /dev/null -w "  HTTP %{http_code}\n" "https://node2.aegis-security.ru:8443/federation/merkle" || true
echo "Public :443/federation/ should be gone (404):"
curl -s -o /dev/null -w "  HTTP %{http_code}\n" "https://node2.aegis-security.ru/federation/merkle" || true

echo ""
echo "[mtls] Done. Run smoke:"
echo "  export FEDERATION_SHARED_SECRET=\$(ssh ${USER}@${PRIMARY} grep FEDERATION_SHARED_SECRET /etc/aegis/agent.env | cut -d= -f2)"
echo "  ./deploy/smoke/integration-federation-prod.sh"
