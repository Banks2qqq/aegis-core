# Scout 2.0 — план (скорректированный)

## Принципы

1. **Один запуск SCOUT** → полная автономная цепочка: сбор → обогащение → Critic → Ingest → Fusion → Healing (фон) → Deception.
2. **Человек в центре:** War Room на русском, сводка «что нашли / что сделали», ссылки, без сырого JSON.
3. **Все открытые источники** подключаются через единый реестр `scout_intel`; без ключа — пропуск с понятным статусом, не падение всего цикла.

## Источники (реестр)

| ID | Источник | Тип | Сейчас |
|----|----------|-----|--------|
| `fstec_bdu` | bdu.fstec.ru | RU gov | ✅ |
| `threatfox` | threatfox.abuse.ch | IOC API | ✅ |
| `urlhaus` | urlhaus.abuse.ch | URL API | ✅ |
| `malwarebazaar` | bazaar.abuse.ch | hash API | ✅ |
| `mitre_attack` | attack.mitre.org | TTP теги | ✅ обогащение |
| `otx` | otx.alienvault.com | Pulse | 🔑 `OTX_API_KEY` |
| `virustotal` | virustotal.com | hash/IP | 🔑 `VT_API_KEY` |
| `xforce` | exchange.xforce.ibmcloud.com | TI | 🔑 `XFORCE_API_KEY` |
| `talos` | talosintelligence.com | IP blocklist | ✅ mirror/file (`TALOS_BLOCKLIST_PATH`) |
| `fortiguard` | fortiguard.com | Outbreak RSS | ✅ |
| `safe_surf` | safe-surf.ru | НКЦКИ RSS | ✅ |
| `pt_analytics` | ptsecurity.com/... | аналитика | 📋 фаза 4 |
| `bi_zone`, `facct`, `rt-solar` | блоги | OSINT | 📋 фаза 4 |

## Этапы

### Этап 1 — Автономия + UX + мульти-источник (текущий спринт)
- `scout_intel` hub: параллельный сбор, таймауты, дедуп.
- Abuse.ch (3 API) + ФСТЭК.
- `scout_orchestrator`: фоновый heal по critical.
- War Room: сводка для оператора, статус каждого источника.

### Этап 2 — Обогащение
- CVE / IP / domain / hash из текста.
- MITRE ATT&CK (ключевые слова + позже STIX).
- Теги: ransomware, apt, initial-access, …
- Улучшенная дедупликация по `content_hash` + IOC.

### Этап 3 — API-источники и агрегаторы
- OTX, VirusTotal (env).
- Talos / Fortinet (по доступным открытым фидам).

### Этап 4 — Устойчивость
- Анти-дубль запуска SCOUT.
- Метрики Prometheus по источникам.
- DR / кэш MITRE локально.

## Цепочка (автономная)

```text
[SCOUT Hub] → findings[]
     → enrich (CVE, MITRE, tags)
     → Critic → Inquisitor → Ingest (KB)
     → Fusion
     → critical? → spawn Healing
     → Deception (threat_level)
     → [WAR ROOM] сводка для человека
```
