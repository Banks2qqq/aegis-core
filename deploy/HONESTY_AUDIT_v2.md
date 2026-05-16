# AEGIS — Honesty audit v2 (branch A deployed)

**Date:** 2026-05-16 · **Gate:** `deploy/smoke/honesty-gate.sh`

## Verified (code + VPS smokes)

| Claim | Evidence |
|-------|----------|
| Real Docker sandbox for healing | `integration-heal-sandbox-real.sh` PASS; metric `aegis_healing_sandbox_duration_seconds > 0` |
| Honeypot Docker listener | `integration-deception-h2.sh`; logs `DeceptionRuntime: docker listener` |
| HITL heal queue | `integration-heal-hitl.sh`; `/api/heal/run` → `pending_hitl` |
| Hashed API keys | SQLite `api_keys`; `test-key-*` → 401 when `AEGIS_DEV_MODE=0` |
| Public metrics | `https://…/metrics` via nginx + JWT |
| Landing metrics | `GET /api/status/public` (real BDU/fusion/federation counts) |
| Pilot form | `POST /api/pilot` → `data/pilot_requests/*.json` + audit |
| Federation | 2 nodes, mTLS 8443, sync; chaos 6/6 |
| Scout intel | ≥8 sources when scout run; hub metrics otherwise |
| Demo / ReAct / Godmode E2E | `integration-demo-e2e.sh` — status, HITL 409, ReAct complete, audit-tail, air-gap toggle, agents |

## Primary vs secondary (intentional)

| Node | `AEGIS_HEAL_APPLY` | `AEGIS_CONTAIN_ENFORCE` | Role |
|------|-------------------|-------------------------|------|
| Primary (178…) | 0 | 0 | prod dry-run heal/contain |
| Secondary (93…) | 1 | 1 | staging enforcement |

## Do not oversell

- Firecracker honeypots: **not** used; Docker nginx only.
- Full network kill-switch: policy + optional iptables, not guaranteed for all workloads.
- Scout/LLM: degraded without external API keys (sources show `skipped`).
- Raft: metrics/orchestration; federation SLA is sync-based.

## Rating (honest)

| Plane | Score |
|-------|-------|
| Data / Scout | 9/10 |
| Action (sandbox, HITL, contain/heal flags) | 9/10 |
| Ops / Federation | 9.5/10 |
| Marketing vs prod | 8.5/10 after H4 landing fix |

**Branch A Definition of Done:** `honesty-gate.sh` PASS on both VPS + `pilot-honest-10-finalize.sh`.
