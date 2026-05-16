# AEGIS Pilot — Honesty audit (PR6.5, one-pager)

> **Superseded for branch A (H1–H8):** see [`HONESTY_AUDIT_v2.md`](HONESTY_AUDIT_v2.md) and `deploy/smoke/honesty-gate.sh` (green on both VPS May 2026).

**Purpose:** what the pilot *actually* gets vs marketing language.

## Strengths (verified in code + smoke)

- **Data plane:** SQLite KB, BDU ingest path, fusion view, audit trail hooks.
- **Action plane:** contain policy + optional host enforcement flags; healing patch applier behind `AEGIS_HEAL_APPLY`; ReAct wired with LLM/key provider.
- **Ops plane:** agent registry, Raft metrics snapshot, federation sync + merkle repair + peer token auth.
- **Security posture:** JWT on dashboard APIs; federation routes fail-closed in production without shared secret; contain/heal gated by env flags.

## Honest limitations (do not oversell)

| Area | Limitation |
|------|------------|
| Federation mTLS | Client cert on outbound calls; **ingress mTLS** is nginx/ops, not fully automatic in agent. |
| Scout / LLM | Depends on keys and network; air-gapped / missing key degrades gracefully but features reduce. |
| Contain | Full network isolation is **policy + optional iptables markers**, not a guaranteed kill switch for all workloads. |
| Healing | Real file apply is opt-in (`AEGIS_HEAL_APPLY`); default paths favor safety. |
| Multi-node E2E | **Code + local smoke** validated; **production** two-node still needs second host + shared secret + ops verification. |
| Auth.rs | **Updated (H5):** hashed keys in SQLite `api_keys`; `test-key-*` blocked when `AEGIS_DEV_MODE=0`. |

## Pilot success criteria (suggested)

1. Restore from backup drill completes on staging.
2. `deploy/smoke/smoke-api.sh` passes against pilot URL (with real JWT key).
3. Federation: two nodes, `merkle_match` after controlled KB change.
4. Runbook walkthrough: restart, log tail, contain dry-run, federation token rotation.

## Rating snapshot (subjective)

Data ~9/10 · Action ~8.5–9/10 (with flags) · Ops ~9.5/10 · Federation code ~9/10 · **Pilot-ready with stated caveats.**
