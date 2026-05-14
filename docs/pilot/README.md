# AEGIS Pilot Package

## Что такое AEGIS
**AEGIS** — Zero-Trust платформа для автономного анализа инцидентов и корреляции угроз (ReAct++ агент + Threat Intelligence + Fusion Engine) с обязательным **Human-in-the-Loop** контролем для критических действий и неизменяемым **Audit Trail**.

## Ключевые возможности (пилот)
- **Air-Gapped / Local LLM**: запуск без внешних вызовов (локальные модели Ollama/vLLM).
- **ReAct++ агент**: пошаговые действия с оценкой **CriticAgent** (security_risk / utility), Kill Switch и MCTS-ранжированием.
- **Threat Hunter + Fusion Engine**: сбор находок и стриминговая корреляция IOC/событий.
- **Human-in-the-Loop**: подтверждение человека для GOD MODE и эскалаций риска.
- **Immutable Audit Trail**: append-only журнал `audit.log` с hash-chain (tamper-evident).
- **HTTP/WebSocket API**: для подключения дашборда и получения live-событий.

## Быстрый старт (минимальный)
Точка входа пилота: **`agent-cli`** (backend).

```bash
cd backend
CARGO_TARGET_DIR=target cargo build --bin agent-cli
./target/debug/agent-cli --config config.yaml
```

## Air-Gapped режим (рекомендуется для пилота)
В Air-Gapped режиме AEGIS:
- **не вызывает внешние LLM**;
- **не использует внешние источники Threat Intel**;
- отключает сетевые инструменты (например, `fetch_url`);
- использует локальные модели (Ollama/vLLM).

Пример `config.yaml` (минимум):

```yaml
mode: pilot
llm:
  mode: airgapped
  local_base_url: "http://localhost:11434/v1"
  fallback_enabled: false
  default_model: "qwen2:14b-instruct-q5_K_M"
security:
  air_gapped: true
  human_in_the_loop: true
  god_mode_safety_level: strict
database:
  sqlite_path: "./data/aegis.db"
  qdrant_url: "http://localhost:6334"
audit:
  enabled: true
  log_path: "./data/audit.log"
  immutable: true
```

Запуск:

```bash
cd backend
./target/debug/agent-cli --config config.yaml
```

## Что будет сделано в пилоте (ожидаемый результат)
- Настройка контуров запуска (pilot/production-like), включая Air-Gapped.
- Демонстрация цепочки контроля:
  - критические действия → **эскалация** → **подтверждение человека**;
  - неизменяемый аудит: кто/что/когда/было ли одобрено.
- Интеграция с существующей сетью/процессами заказчика (на уровне API и runbook).
- Проверка базовых KPI:
  - стабильность;
  - предсказуемость поведения в запретных режимах;
  - воспроизводимость аудита.

## Требования к инфраструктуре (базово)
- ОС: Linux/macOS для демо; для пилота — Linux (рекомендовано).
- CPU: 4+ cores (рекомендовано 8+).
- RAM: 16+ GB (для локальных LLM желательно 32+ GB).
- Диск: 10+ GB (логи/БД/артефакты), больше при длительном пилоте.
- Локальный LLM:
  - Ollama или vLLM, доступный по `local_base_url`.

## Как подготовиться к демонстрации (рекомендуемый порядок)
1. **Показать режимы контроля**:
   - `/pilot-info`
2. **Показать Air-Gapped** (если включён):
   - баннер Air-Gapped при старте
   - подтвердить, что сетевые инструменты отключены
3. **Показать Human-in-the-Loop**:
   - `/code <любой тестовый запрос>` → ожидается подтверждение `[y/N]`
4. **Показать ReAct++ безопасное поведение**:
   - `/react <mission>` → при рискованных шагах должен быть `ESCALATE` вместо выполнения
5. **Показать доказуемость (Audit Trail)**:
   - открыть `audit.log` и показать `prev_hash/hash` цепочку для событий HITL и GOD MODE

## Рекомендуемый сценарий демонстрации (10–15 минут)
1. **Старт и режимы**:
   - запуск `agent-cli` с `mode: pilot` и `air_gapped: true`
   - команда `/pilot-info` (в одной консоли, без “магии”)
2. **Автосценарий**:
   - команда `/demo` (показывает Air-Gapped, HITL, 1–2 ReAct шага и хвост audit.log)
3. **Управляемая эскалация**:
   - `/code <task>` → демонстрация обязательного подтверждения человека
4. **Проверка доказательности**:
   - показать 5–10 последних строк `audit.log` и объяснить `prev_hash/hash`

## Known Limitations (Pilot)
- **Audit Trail tamper-evident, не WORM**: hash-chain фиксирует факт подмены, но для “неизменяемости по регламенту” нужна интеграция с WORM/KMS/централизованным хранилищем.
- **Качество локального LLM**: зависит от выбранной модели и ресурсов; безопасность обеспечивается Critic/HITL, но качество текста/планов может варьироваться.
- **Offline findings в air-gapped**: демонстрационные/эвристические без внешних TI-источников; в пилоте это используется как пример контролируемого режима.

### Пример вывода `/pilot-info`

```bash
AEGIS Pilot Package v0.9
- Air-Gapped: ENABLED
- Audit Trail: ACTIVE
- Human-in-the-Loop: ENABLED
- Documentation: docs/pilot/
```

## Документы пакета
- `INSTALL.md` — установка (включая air-gapped).
- `ARCHITECTURE.md` — архитектура и схема.
- `THREAT_MODEL.md` — базовый threat model и mitigations.
- `RUNBOOK.md` — запуск/проверка/диагностика.

