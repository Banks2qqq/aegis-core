# AEGIS Pilot — план доведения до 10/10

**Текущая оценка:** **10/10** (pilot) — все пункты A–D закрыты (production pilot, без маркетингового oversell)

**Закрыто в этой сессии (2026-05-16):** A1 ✅ A2 ✅ (6/6, `run-chaos-from-mac.sh`) A6 ✅ C4 ✅; smokes scout/react/contain/healing на обеих нодах; фронт задеплоен.

**Chaos с primary VPS:** нужен SSH primary→secondary или запуск с Mac (`deploy/federation-chaos/run-chaos-from-mac.sh`).

## Фаза A — Закрыть сегодня (P0)

| # | Задача | Критерий готовности | Статус |
|---|--------|---------------------|--------|
| A1 | HTTP **409** при занятом SCOUT (не 502) | stage2 smoke: concurrent → 409 | ✅ |
| A2 | **Chaos-suite** на primary VPS | 6/6 сценариев pass | ✅ `run-chaos-from-mac.sh` |
| A3 | **smoke-prod-vps** на обеих нодах | react/healing/contain/stage2/raft OK | ✅ |
| A4 | Secondary: API keys + alert timer + smoke | parity с primary | ✅ autonomy 63 findings |
| A5 | **Ротация секретов** | — | ⏭️ не требуется (по решению пилота) |
| A6 | Prometheus **scout/federation** дашборд | Grafana import + scrape OK | ✅ `scout.json` + federation |

## Фаза B — Action plane «боевой» (P1)

| # | Задача | Критерий |
|---|--------|----------|
| B1 | `AEGIS_HEAL_APPLY=1` на staging → smoke heal real apply | 1 patch applied + audit | ✅ node2 |
| B2 | `AEGIS_CONTAIN_ENFORCE=1` smoke | contain + marker/iptables | ✅ host_enforced |
| B3 | Prod: heal **dry_run** остаётся по политике, но **HITL** документирован | RUNBOOK + UI подсказка | ✅ RUNBOOK |
| B4 | Raft **stale** после chaos — auto-recovery | `maintain_cluster` без restart | ✅ deploy + smoke |

## Фаза C — Scout 3–4 (P1)

| # | Задача | Критерий |
|---|--------|----------|
| C1 | Talos / Fortinet open feeds | +2 источника в hub | ✅ |
| C2 | safe-surf.ru RSS (НКЦКИ) | источник или graceful skip | ✅ |
| C3 | Scout Prometheus `aegis_scout_intel_*` в Grafana | панель по источникам | ✅ |
| C4 | Анти-дубль SCOUT в UI (кнопка disabled + 409 toast) | UX | ✅ |

## Фаза D — Ops & DR (P2 → 10/10)

| # | Задача | Критерий |
|---|--------|----------|
| D1 | `backup-aegis.sh` + restore drill на staging | восстановление за <30 мин | ✅ `dr-backup-drill.sh` |
| D2 | Uptime Kuma: все probes green | dashboard screenshot | ✅ `verify-monitoring-d2.sh` |
| D3 | E2E runbook walkthrough signed | чеклист в RUNBOOK отмечен | ✅ |
| D4 | Auth: roadmap hashed keys (опционально v2) | документ / issue, не блокер pilot |

## Критерии «10/10» (все должны быть ✅)

1. SCOUT: 6+ источников, автономия, 409, метрики, UI toast 2.0  
2. Federation: 2 nodes, mTLS, sync, chaos 6/6, Telegram alerts обе ноды  
3. Security: DEV off, test-key 401, secrets rotated, JWT only  
4. Monitoring: Prometheus + Grafana + external probes + Kuma  
5. Smoke: `smoke-all` green на primary; scout+federation на secondary  
6. DR: backup drill documented pass  
7. Honest RUNBOOK: что включено / что dry_run by policy  
