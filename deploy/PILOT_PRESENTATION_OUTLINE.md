# Pilot deck outline (PR6.5)

Use 8–10 slides; keep claims aligned with `HONESTY_AUDIT_PR6.md`.

1. **Problem** — autonomous defence needs data + action + ops in one loop.
2. **What AEGIS is** — Rust agent + dashboard; KB, fusion, agents, federation.
3. **Live demo flow** — login → overview → scout (timeboxed) → threats → contain (synthetic id ok).
4. **Data plane** — BDU/intel → KB; fusion clusters; audit.
5. **Action plane** — contain policy; heal patches (env-gated); ReAct status.
6. **Ops plane** — registry, Raft snapshot, federation sync + repair + smoke.
7. **Security & Zero-Trust** — JWT, federation tokens, fail-closed prod, optional enforcement.
8. **Deployment** — systemd, nginx, `deploy-all.sh`, backup/restore scripts.
9. **Limitations (honest)** — mTLS termination, LLM dependency, second-node E2E on prod.
10. **Pilot ask** — success criteria, timeline, who operates backups & secret rotation.

Speaker notes: always say “with env flags” when discussing heal/contain enforcement.
