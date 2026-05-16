# AEGIS Operator Runbook (PR6.3)

## Production deploy (100% path)

```bash
# One-shot: VPS build, frontend, alerts, autostart, smoke gate
./deploy/production-deploy.sh

# Or step-by-step:
./deploy/federation-pilot-deploy.sh
./deploy/production-enable-services.sh
./deploy/monitoring/install-monitoring-primary.sh   # needs Docker on primary
./deploy/federation-alert/install-alert-secondary.sh
./deploy/production-ssh-secondary.sh              # one-time SSH key
```

**Autostart (mandatory):** `systemctl is-enabled aegis-agent nginx` → `enabled` on both nodes.

**Monitoring (primary, localhost):** Prometheus `:9090`, Grafana `:3000`, Uptime Kuma `:3001` — SSH tunnel or nginx reverse proxy.

**Secrets:** never commit `*.exp` passwords; use `/etc/aegis/federation-alert.env` (600).

**API keys (production):** `AEGIS_DEV_MODE=0` — test-keys disabled. Retrieve keys:
```bash
ssh root@178.236.16.101 'grep AEGIS_.*_API_KEY /etc/aegis/agent.env'
```
- `AEGIS_MONITOR_API_KEY` — alerts, Prometheus, smoke (server-side only)
- `AEGIS_DASHBOARD_API_KEY` — operator login in dashboard UI

**Telegram after BotFather /revoke:**
```bash
echo 'NEW_TOKEN' > deploy/federation-alert/.telegram-token && chmod 600 deploy/federation-alert/.telegram-token
./deploy/federation-alert/apply-telegram-token.sh
```

**Raft vs federation:** см. `ARCHITECTURE.md` — SLA строится на federation sync, не на Raft leader. После chaos/downtime Raft самовосстанавливается (`maintain_cluster` каждые 30s); проверка: `GET /api/raft/status` → `leader_id` не null, ноды `live`.

## Healing & contain (policy / HITL)

| Flag | Production (pilot) | Staging / smoke |
|------|-------------------|-----------------|
| `AEGIS_HEAL_APPLY` | `0` (dry-run) | `1` только после явного одобрения |
| `AEGIS_CONTAIN_ENFORCE` | `0` | `1` для `integration-scout-contain.sh` |

- **Heal:** SCOUT ставит `heal_q` в очередь; без `AEGIS_HEAL_APPLY=1` патчи не применяются на хосте — только audit + War Room.
- **Contain:** smoke создаёт тестовый cluster; с `ENFORCE=0` — marker без iptables.
- **HITL:** критические действия (heal apply, federation sync_all) — только оператор с dashboard key; см. God Mode / audit.log.

Проверка prod smoke (без полного SCOUT):

```bash
export BASE_URL=https://aegis-security.ru   # или node2
source /etc/aegis/agent.env
export SMOKE_API_KEY="$AEGIS_MONITOR_API_KEY"
bash /opt/aegis/deploy/smoke/smoke-prod-vps.sh
# полный SCOUT: RUN_SCOUT_AUTONOMY=1 bash .../smoke-prod-vps.sh
```

**Branch A — honesty gate (H1–H8, с Mac):**

```bash
./deploy/pilot-honest-10-finalize.sh
# только gate на VPS: /opt/aegis/deploy/smoke/honesty-gate.sh
# scout sources_ok≥8: HONESTY_RUN_SCOUT=1 honesty-gate.sh
```

| Node | URL | `HEAL_APPLY` | `CONTAIN_ENFORCE` |
|------|-----|--------------|-------------------|
| Primary | `aegis-security.ru` | `0` | `0` |
| Secondary | `node2.aegis-security.ru` | `1` | `1` |

- Sandbox: `AEGIS_SANDBOX_RUNTIME=docker` — реальный `docker run`, не `duration=0.00s`.
- Deception: Docker nginx listener (`POST /api/deception/deploy`), не Firecracker.
- HITL: `/dashboard/healing` + `GET/POST /api/heal/pending|approve|reject`.
- Аудит: `deploy/HONESTY_AUDIT_v2.md`.

**Scout C1 — Talos + FortiGuard:**
```bash
./deploy/scout-sync-talos-feed.sh    # mirror → /opt/aegis/feeds/talos-ip-blacklist.txt
./deploy/scout-c1-deploy.sh          # build + smoke
# Talos official URL часто 403 с VPS (Cloudflare) — используйте TALOS_BLOCKLIST_URL или локальный файл
./deploy/scout-sync-fortiguard-rss.sh  # node2 может не достучаться до fortiguard.com по TLS
./deploy/scout-c2-deploy.sh            # НКЦКИ safe-surf.ru RSS (https://safe-surf.ru/rss)
```

**Staging action plane** (secondary `node2` — real apply/enforce; primary остаётся dry-run):

```bash
./deploy/staging-action-plane-deploy.sh
# или только флаги: ./deploy/staging-enable-action-plane.sh
```

## Architecture (typical VPS)

- **Nginx** terminates TLS, serves static dashboard from `/var/www/aegis/html`, proxies API/WebSocket to `127.0.0.1:8080`.
- **agent-cli** runs as `systemd` unit `aegis-agent`, `WorkingDirectory=/opt/aegis/backend`, reads `config.yaml` and `/etc/aegis/agent.env`.

## Start / stop / logs

```bash
sudo systemctl status aegis-agent
sudo systemctl restart aegis-agent
sudo journalctl -u aegis-agent -f --no-pager
```

## Binary update (avoid “Text file busy”)

```bash
sudo systemctl stop aegis-agent
sudo cp /opt/aegis/backend/target/release/agent-cli /opt/aegis/bin/agent-cli
sudo chmod 755 /opt/aegis/bin/agent-cli
sudo systemctl start aegis-agent
```

Or use `deploy/deploy-all.sh` from a dev machine (SSH + tarball + remote `cargo build`).

## Health checks

```bash
# Through nginx (HTTPS): use exact locations so static `location /` does not steal /health
curl -sS https://aegis-security.ru/health
curl -sS https://aegis-security.ru/api/health   # alias → same JSON as /health
curl -sS http://127.0.0.1:8080/health
# JWT flow
curl -sS -X POST http://127.0.0.1:8080/api/login \
  -H "Content-Type: application/json" \
  -d '{"api_key":"<your-key>"}'
```

## Automated smoke / integration (PR6.1)

- `deploy/smoke/smoke-api.sh` — health + JWT + core GET routes.
- `deploy/smoke/integration-scout-contain.sh` — contain path (optional `SKIP_SCOUT=0` enables full scout).
- `deploy/smoke/smoke-all.sh` — full bundle; set **`SKIP_FEDERATION=1`** if you only have one listener on the host.

## Federation pilot readiness (3 tracks)

| Priority | Track | Scripts / API |
|----------|--------|-----------------|
| 1 | Resilience / chaos | `deploy/federation-chaos/run-chaos-suite.sh` (`CHAOS_CONFIRM=1`) |
| 2 | Observability | `/metrics` (`aegis_federation_*`), `/api/federation/metrics`, dashboard Federation Ops |
| 3 | Alerts | `deploy/federation-alert/check-federation-alert.sh` + systemd timer |

### Chaos suite (destructive — production only with care)

```bash
export CHAOS_CONFIRM=1
export VPS_PASSWORD='...'   # secondary SSH
export CHAOS_SCENARIOS=secondary_down,network_partition,cert_break,recovery
./deploy/federation-chaos/run-chaos-suite.sh
# results: /tmp/aegis-chaos-results/chaos-run.jsonl
```

Scenarios: secondary/primary stop, iptables block :8443, break client cert, long downtime, recovery timing.

### Alerts (cron / systemd)

```bash
# Full pilot deploy (builds agent on Linux VPS — never upload Mac binary):
./deploy/federation-pilot-deploy.sh

# Alerts only:
mkdir -p /opt/aegis/deploy/{federation-alert,smoke}
# copy check-federation-alert.sh, lib.sh, *.service, *.timer
systemctl enable --now aegis-federation-alert.timer
# Telegram (@AEGIS_GOD_BOT):
export ALERT_TELEGRAM_BOT_TOKEN='...'   # from @BotFather — never commit
./deploy/federation-alert/setup-telegram-alert.sh
# Or on VPS: send /start to bot, then:
#   /opt/aegis/deploy/federation-alert/wait-telegram-chat.sh
# chat_id is auto-saved to /etc/aegis/federation-alert.env

# Full prod smoke from VPS (mTLS certs local):
ssh root@178.236.16.101 /opt/aegis/deploy/smoke/integration-federation-prod-vps.sh
```

Logs: `journalctl -t aegis-federation-alert`

## Federation (PR5+) — two ports + mTLS

| Port | Purpose |
|------|---------|
| **443** | Public HTTPS: dashboard, `/api/*`, `/health` (no client cert) |
| **8443** | Federation only: `ssl_verify_client on`, Federation CA in `/etc/aegis/federation/ca.pem` |

**Config (`federation.peers`):**

- `url` — public base for health (e.g. `https://node2.aegis-security.ru`)
- `federation_url` — mTLS listener (e.g. `https://node2.aegis-security.ru:8443`)

**Secrets & certs**

- **`FEDERATION_SHARED_SECRET`** in `/etc/aegis/agent.env` on every node (same value).
- Generate CA + client certs: `./deploy/generate-federation-mtls.sh` → `deploy/distribute-federation-mtls.sh`
- Apply nginx + agents: `./deploy/enable-federation-mtls-prod.sh` (no `AEGIS_FEDERATION_INSECURE_TLS` in production)

**Logs**

- Agent: `journalctl -u aegis-agent -f | grep -i 'Federation mTLS'`
- Nginx (after `deploy/nginx-federation-mtls-log.conf` in `/etc/nginx/conf.d/`):

  `tail -f /var/log/nginx/aegis-federation-mtls.log`

**Monitoring**

```bash
export FEDERATION_SHARED_SECRET=$(ssh root@PRIMARY grep FEDERATION_SHARED_SECRET /etc/aegis/agent.env | cut -d= -f2)
./deploy/smoke/check-federation-mtls-port.sh
./deploy/smoke/integration-federation-prod.sh
```

**Smoke**

- Local two-node: `deploy/pr5-federation-smoke.sh`
- Full bundle: `BASE_URL=http://127.0.0.1:8080 ./deploy/smoke/smoke-all.sh` — `SKIP_FEDERATION=1` skips federation block

## Backup & restore (PR6.2)

```bash
sudo ./deploy/backup-aegis.sh /root/aegis-backups/manual-1
# optionally include env (secrets):
sudo READ_AGENT_ENV=1 ./deploy/backup-aegis.sh /root/aegis-backups/with-env
sudo ./deploy/restore-aegis.sh /root/aegis-backups/manual-1
```

Dry-run restore: `DRY_RUN=1 sudo ./deploy/restore-aegis.sh ...`

Protect backup directories (`chmod 700`, off-box copy).

**D1 — DR drill (staging / secondary):**

```bash
./deploy/dr-backup-drill.sh
# Проверяет: backup → simulate loss → restore → KB count match → /health < 30 min
```

Чеклист drill: `[x]` backup manifest + sqlite `[x]` restore `[x]` agent active `[x]` smoke `integration-dr-backup.sh`

## Frontend deploy

```bash
cd frontend && npm run build
./deploy_to_vps.sh   # needs SSH; uses frontend/out
```

## When things break

| Symptom | Check |
|---------|--------|
| 502 from Nginx | `systemctl is-active aegis-agent`, `journalctl -u aegis-agent -n 80` |
| JWT 401 | Clock skew, expired token, login body must use `access_token` in clients |
| Federation 401/503 | Missing `FEDERATION_SHARED_SECRET` or wrong token header |
| Federation 403 on :8443 | Client cert not presented or not signed by Federation CA |
| Port 8443 closed | `ufw allow 8443/tcp`, nginx `listen 8443`, cert paths OK |
| Scout slow / fails | LLM keys in `agent.env`, outbound network, air-gapped flags |
| iptables / contain | `AEGIS_CONTAIN_ENFORCE`, run as root-capable service |

## Monitoring D2 (Uptime Kuma + probes)

```bash
./deploy/pilot-10-finalize.sh
# или на primary:
/opt/aegis/monitoring/setup-uptime-kuma.sh
/opt/aegis/monitoring/verify-monitoring-d2.sh
```

Credentials: `/etc/aegis/uptime-kuma.env` (UK_USER / UK_PASS). UI: SSH tunnel `ssh -L 3001:127.0.0.1:3001 root@PRIMARY` → http://127.0.0.1:3001

Prometheus `/metrics` требует JWT — таймер `aegis-prometheus-token.timer` обновляет `/etc/aegis/monitoring/bearer_token` каждые 10 мин.

## Pilot 10/10 — E2E walkthrough (D3)

| # | Проверка | Команда / критерий | ✓ |
|---|----------|-------------------|---|
| 1 | Autostart | `systemctl is-enabled aegis-agent nginx` на обеих нодах | [x] |
| 2 | Health HTTPS | `curl -sf https://aegis-security.ru/health` | [x] |
| 3 | SCOUT 9 sources | `integration-scout-c1.sh` + `integration-scout-c2.sh` | [x] |
| 4 | Federation chaos | `run-chaos-from-mac.sh` 6/6 | [x] |
| 5 | Staging heal/contain | `smoke-staging-action.sh` на node2 | [x] |
| 6 | DR drill | `dr-backup-drill.sh` | [x] |
| 7 | Monitoring | `verify-monitoring-d2.sh` + Prometheus/Grafana | [x] |
| 8 | Prod smoke | `smoke-prod-vps.sh` primary + secondary | [x] |
| 9 | Policy | heal dry_run primary; HITL в RUNBOOK | [x] |

## Contacts / ownership

Document your on-call and where **secrets** live (`/etc/aegis/agent.env`, `JWT_SECRET`, LLM keys).
