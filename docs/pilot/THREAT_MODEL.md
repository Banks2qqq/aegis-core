## Threat Model (Pilot Readiness)

### Scope
Документ описывает базовую модель угроз для пилота AEGIS (контур “показ/оценка”), с допущением Zero-Trust и **Assumed Breach**.

### Assumed Breach
Предполагаем, что:
- часть пользовательских рабочих мест/учётных записей может быть скомпрометирована;
- часть входных данных может быть злонамеренной (включая prompt/tool injection);
- сеть может быть недоверенной (MITM/подмена DNS/прокси/зеркала);
- внешние сервисы недоступны или небезопасны (поэтому в пилоте приоритет — Air-Gapped).

### Key Assets
1. **Целостность и трассируемость действий**
   - `audit.log` (append-only, hash-chain) и события HITL/эскалаций.
2. **Управление режимами безопасности**
   - корректность `config.yaml` (air-gapped/hybrid/cloud), запреты на сетевые инструменты.
3. **Секреты и токены**
   - JWT/ключи доступа, исключение утечек в логи/вывод.
4. **Данные пилота**
   - findings, артефакты, результат работы ReAct++.
5. **Исполняемая среда**
   - запрет/контроль “execute/deploy” и любых опасных системных действий.

### Threat Scenarios + Mitigations
Ниже — приоритетные сценарии угроз для пилота и конкретные меры контроля в AEGIS.

| Scenario | Preconditions / Attack vector | Impact | Controls (AEGIS) | What we verify in pilot |
|---|---|---|---|---|
| Prompt Injection / Tool Injection | Злонамеренный ввод, скрытые инструкции, jailbreak, “tool forcing” | Экcфильтрация данных / попытка выполнения опасных действий | CriticAgent (BLOCK/ESCALATE), Kill Switch, `needs_human_approval`, HITL, `prompt_guard`, allowlist ToolRegistry | Эскалация вместо выполнения + запись в `audit.log` |
| Data Exfiltration (network) | Попытки сетевых вызовов через tools/LLM/подсказки | Утечка данных/артефактов | Air-Gapped режим, отключение сетевых инструментов (`fetch_url`), аудит критических событий | Баннер Air-Gapped + отсутствие сетевых tools + аудит |
| Unauthorized “Deploy/Execute” | Автогенерация деплоя/команд выполнения | Изменение среды, риск простоя | HITL gate для GOD MODE, блок исполнения при high-risk, Audit Trail | `/code` требует подтверждение + лог HITL |
| Tampering with logs | Удаление/подмена событий аудита | Потеря доказательности | Append-only JSONL + hash-chain (`prev_hash/hash`), best-effort `sync_data` при `immutable=true` | Проверка непрерывности цепочки на последних N строках |
| Supply chain / dependency risk | Уязвимости crates / вредоносные обновления | Компрометация сборки | Рекомендации GOD MODE: `cargo-audit`/`cargo-deny`, контрольный процесс сборки в пилоте | Показ runbook-процесса и контрольных точек |
| Local LLM reliability / drift | Ошибки локальной модели, деградация качества | Ложные выводы/нестабильные планы | Critic + HITL, ограничение инструментов, аудит решений | Демонстрация “guardrails”: даже при странном выводе нет автодействий |

### Residual Risks (пилот)
Остающиеся риски, которые не устраняются полностью в рамках пилота:
- Hash-chain **tamper-evident**, но не WORM/KMS-подпись (для прод — отдельная интеграция).
- Локальный LLM остаётся вероятностным компонентом; контроль обеспечивается Critic/HITL, но качество вывода не гарантировано.
- Внешние угрозы уровня ОС/гипервизора вне зоны ответственности пилота (рекомендуется hardening окружения заказчика).

### Recommendations for Pilot
1. **Запуск в Air-Gapped** как дефолтный режим демонстрации.
2. Включить:
   - `security.human_in_the_loop: true`
   - `security.god_mode_safety_level: strict`
   - `audit.enabled: true`, `audit.immutable: true`
3. На демонстрации обязательно показать:
   - `/pilot-info` (режимы контроля)
   - `/code <task>` (HITL подтверждение + audit)
   - `/react <mission>` с high-risk шагом (ESCALATE вместо выполнения)
   - `audit.log` (цепочку `prev_hash`/`hash`)

### What we show in pilot (коротко)
- **Контур запуска**: Air-Gapped включён, сетевые инструменты отключены.
- **Контроль действий**: GOD MODE и high-risk шаги требуют HITL или эскалируются.
- **Доказуемость**: `audit.log` содержит события HITL/решений Critic и проверяемую hash-chain.

