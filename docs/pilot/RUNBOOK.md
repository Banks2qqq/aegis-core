## Runbook (Pilot)

### 0) Подготовка
- Проверьте `backend/config.yaml`
- Для Air-Gapped: убедитесь, что локальный LLM отвечает на `/v1/chat/completions`
- Убедитесь, что путь аудита доступен (`audit.log`)

### 1) Сборка и запуск

```bash
cd backend
CARGO_TARGET_DIR=target cargo build --bin agent-cli
./target/debug/agent-cli --config config.yaml
```

### 2) Базовая проверка функций
В CLI:
- `/fusion` — показать статистику корреляции
- `/research <target>` — OSINT анализ (в air-gapped будет зависеть от режима LLM, но без внешних источников TI)
- `/react <mission>` — ReAct++ миссия (при high-risk должна быть эскалация)
- `/god` — GOD MODE рекомендации (в strict требует HITL)
- `/code <task>` — GOD MODE код/аудит (в strict требует HITL)
- `/pilot-info` — печать путей к документации пилота

### 3) Проверка HITL
- Выполните `/code test`
- Ожидается запрос `[y/N]` и запись в аудит:
  - `hitl_prompt`
  - `hitl_response`

### 4) Проверка Audit Trail
Проверьте `backend/data/audit.log` (или путь из конфига).
Ожидается JSONL формат, каждая строка содержит:
- `prev_hash`
- `hash`

### 5) Диагностика
- Если локальный LLM недоступен: проверьте `llm.local_base_url`
- Если audit не создаётся: проверьте `audit.log_path` и права на каталог

