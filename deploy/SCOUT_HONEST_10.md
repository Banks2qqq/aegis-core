# Scout — честные 10/10 (продуктовый отчёт)

**Цель:** один запуск SCOUT → понятный структурированный результат + прозрачная автономия (без маркетингового oversell).

## Definition of Done

| # | Критерий | Gate |
|---|----------|------|
| S1 | 6+ источников OK в ответе API | `integration-scout-honest-10.sh` |
| S2 | `report.executive_summary_ru` + `top_findings` (все источники, не только BDU) | API + War Room UI |
| S3 | `report.reactions` с disposition (`hitl_queue` / `auto_apply_eligible`) | API + UI |
| S4 | `report.enrichment` (IOC/CVE/IP/domain/hash/dedup) | API + UI |
| S5 | `report.autonomy.description_ru` отражает `AEGIS_HEAL_APPLY` | API + UI |
| S6 | Анти-дубль SCOUT (409) | concurrent check в smoke |
| S7 | Фоновый heal по critical (sandbox → HITL или apply) | `integration-scout-c1.sh` + логи |

## API

`POST /api/scout` → поле `report`:

```json
{
  "report": {
    "executive_summary_ru": "...",
    "top_findings": [...],
    "reactions": [...],
    "enrichment": { "total_iocs", "total_cves", ... },
    "autonomy": { "heal_apply_enforced", "description_ru" }
  },
  "sources": [{ "id", "label", "status", "count", "note" }]
}
```

## UI

- **War Room** (`/dashboard/overview`): `ScoutReportPanel` после SCOUT
- Toast остаётся кратким; полный отчёт — на Overview

## Политика пилота

- **Primary** (`HEAL_APPLY=0`): автореакция = sandbox + **HITL очередь** (`hitl_queue`)
- **Secondary** (`HEAL_APPLY=1`): возможно автоприменение после sandbox (`auto_apply_eligible`)

## Smoke

```bash
export BASE_URL=https://aegis-security.ru
export SMOKE_API_KEY=...
bash deploy/smoke/integration-scout-honest-10.sh
```
