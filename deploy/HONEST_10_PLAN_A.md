# AEGIS — план «честные 10/10» (ветка A: реальный sandbox)

**Выбор:** ветка **A** — реальная изоляция (Docker → опционально Firecracker), без misleading-логов.

**Текущее:** pilot ops ~9.5/10, truth in action plane ~6/10 (sandbox/honeypot simulated).

**Цель:** каждый claim в UI/логах/docs подтверждён кодом + `honesty-gate.sh` PASS.

---

## PR roadmap

| PR | Название | Файлы (основные) | Gate |
|----|----------|------------------|------|
| **H1** | Real Docker sandbox for healing | `sandbox_executor.rs`, `healing_orchestrator.rs`, `metrics.rs` | ✅ `integration-heal-sandbox-real.sh` PASS primary + secondary (2026-05-16) |
| **H2** | Honeypot: Docker listener (честный deception) | `deception_runtime.rs`, `honeypot_manager.rs` | ✅ `integration-deception-h2.sh` PASS primary + secondary (2026-05-16) |
| **H3** | HITL heal approve API + UI | `heal_queue.rs`, `server.rs`, godmode | ✅ `integration-heal-hitl.sh` |
| **H4** | Landing truth + pilot API | `page.tsx`, `api_status/public` | ✅ реальные метрики + pilot persist |
| **H5** | Hashed API keys | `api_key_store.rs`, `auth.rs` | ✅ `integration-auth-h5.sh` |
| **H6** | Nginx `/metrics` + public status | `nginx-aegis-*.conf` | ✅ `integration-nginx-metrics-h6.sh` |
| **H7** | Demo/ReAct/Godmode E2E smoke | `integration-demo-e2e.sh` | all 200 |
| **H8** | `honesty-gate.sh` + finalize | `deploy/smoke/honesty-gate.sh`, `pilot-honest-10-finalize.sh` | ✅ |

**Критический путь:** H1 → H2 → H3 → H8. H4–H7 параллельно после H1.

---

## H1 — Real Docker sandbox (детально)

### Поведение
1. Патч пишется в `data/sandbox/<id>/patch.txt`.
2. `docker run --rm --network none --read-only --memory 256m --cpus 1 -v ...:ro alpine:3.20` выполняет validator:
   - файл не пустой;
   - нет denylist (`rm -rf /`, `mkfs`, `curl | sh`, …);
   - для `[Config]` — только текст, без бинарных NUL.
3. Exit 0 → `sandbox_passed=true`, метрика `aegis_healing_sandbox_result_total{result="pass"}`.
4. Exit ≠ 0 → patch **не** apply (даже при Low risk).

### Env (VPS `/etc/aegis/agent.env`)
```bash
AEGIS_SANDBOX_RUNTIME=docker   # docker | off
AEGIS_SANDBOX_IMAGE=alpine:3.20
AEGIS_SANDBOX_TIMEOUT_SECS=120
```

### Off / fallback
- `AEGIS_SANDBOX_RUNTIME=off` — только AST formal verify (документировано), метрика `result=skipped`.
- Docker недоступен → WARN в лог, `result=unavailable`, heal blocked для High/Critical.

### Smoke
```bash
BASE_URL=... SMOKE_API_KEY=... ./deploy/smoke/integration-heal-sandbox-real.sh
```
- POST heal path triggers sandbox
- Prometheus: `aegis_healing_sandbox_duration_seconds > 0`
- Bad patch injection test → `sandbox_failed`

---

## H2 — Honeypot Docker listener ✅

### Реализовано
- `deception_runtime.rs`: `docker run -d -p 127.0.0.1:<port>:80` + `nginx:alpine` + fake admin HTML.
- Canary в HTML comment; `POST /api/deception/canary-trip` → audit + auto-deploy hook.
- Лог: `DeceptionRuntime: docker listener` (без ложного Firecracker).
- API: `POST /api/deception/smoke-deploy`, метрика `aegis_deception_listener_total{runtime,result}`.
- Smoke: `./deploy/smoke/integration-deception-h2.sh` (deploy: `./deploy/h2-deception-deploy.sh`).

### Опционально v2 (отдельный спринт)
- Firecracker на KVM-enabled VPS (проверка `kvm-ok` на Beget).

---

## H3 — HITL heal ✅

| Endpoint | Назначение |
|----------|------------|
| `GET /api/heal/pending` | очередь после sandbox pass |
| `POST /api/heal/approve` | human approve → apply на диск (`enforce=true`) |
| `POST /api/heal/reject` | audit reject |
| `POST /api/heal/run` | полный цикл orchestrator → HITL или apply |

UI: God Mode — панель HITL HEAL QUEUE (Approve/Reject).  
Метрика: `aegis_heal_hitl_total{action,risk}`.  
Smoke: `./deploy/smoke/integration-heal-hitl.sh` (deploy: `./deploy/h3-hitl-deploy.sh`).

---

## H4 — Landing ✅

- Формы → `POST /api/pilot` + persist `data/pilot_requests/*.json` + audit.
- Hero stats → `GET /api/status/public` (BDU, fusion, federation, heal ready).
- Убраны маркетинговые «42K» / симуляция; CTA → панель управления.

---

## H5 — Auth ✅

- SQLite `api_keys` (SHA-256 с pepper = JWT_SECRET).
- `agent-cli hash-key <plaintext>` — офлайн хеш.
- При старте: `migrate_env_keys_once` из `AEGIS_MONITOR_API_KEY` / `AEGIS_DASHBOARD_API_KEY`.
- Login: только hashed lookup; `test-key-*` только при `AEGIS_DEV_MODE=1`.
- Smoke: `./deploy/smoke/integration-auth-h5.sh`

---

## H6 — Nginx metrics ✅

- `location = /metrics` и `/api/status/public` в `nginx-aegis-full.conf` / `node2`.
- Smoke: `./deploy/smoke/integration-nginx-metrics-h6.sh` (HTTPS + JWT).

---

## H8 — honesty-gate.sh (финальный) ✅

Скрипты:
- `deploy/smoke/honesty-gate.sh` — на каждой VPS
- `deploy/pilot-honest-10-finalize.sh` — обе ноды + federation + chaos 6/6
- `deploy/HONESTY_AUDIT_v2.md` — итоговый аудит

Проверки: health, journal (no fake sandbox / no Firecracker honeypot logs), api_keys, test-key 401, metrics H1–H3, federation peers, pilot POST, branch-A integration bundle, optional `HONESTY_RUN_SCOUT=1` для sources_ok≥8.

---

## Deploy order (VPS)

1. H1 backend → build both nodes → `integration-heal-sandbox-real.sh` on **secondary** first
2. H2 → verify port + canary
3. H3 → staging approve flow
4. H4–H6 → frontend + nginx reload
5. H8 full gate

---

## Оценка сроков

| PR | Дни (1 dev) |
|----|-------------|
| H1 | 3–5 |
| H2 | 3–4 |
| H3 | 2–3 |
| H4–H6 | 3–5 (параллель) |
| H7 | 2 |
| H8 | 2 |
| **Итого** | **~4–6 недель** календарно, **~3 недели** при параллели |

---

## Definition of Done «честные 10/10»

1. Sandbox: реальный docker run, duration > 0, fail-closed на toxic patch.
2. Honeypot: реальный TCP listener (docker), canary trip в audit.
3. UI/RUNBOOK = поведение prod (primary dry-run подписан).
4. `honesty-gate.sh` PASS на обоих VPS после deploy.
5. `HONESTY_AUDIT` v2 без красных строк.
6. Демо-скрипт 30 мин без расхождений с логами.
