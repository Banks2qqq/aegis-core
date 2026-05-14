## Установка (Pilot)

### Вариант A — стандартный (с доступом к нужным репозиториям)

#### 1) Зависимости
- Rust toolchain: **Rust 1.85+**
- (опционально) `protoc` для сборки gRPC, если вы меняете `.proto`
- Локальная модель (для air-gapped/local): **Ollama** или **vLLM**

#### 2) Сборка backend

```bash
cd backend
CARGO_TARGET_DIR=target cargo build --bin agent-cli
```

#### 3) Конфигурация
Создайте `backend/config.yaml` (пример в `docs/pilot/README.md`).

#### 4) Запуск

```bash
cd backend
./target/debug/agent-cli --config config.yaml
```

---

### Вариант B — Air-Gapped (строго без внешних вызовов)

#### 1) Подготовка локального LLM
AEGIS ожидает OpenAI-совместимый endpoint:
- `http://localhost:11434/v1` (часто используется как прокси-совместимость)

Проверьте, что `/v1/chat/completions` доступен и отвечает.

#### 2) Конфиг Air-Gapped
Минимальные параметры:
- `security.air_gapped: true`
- `llm.mode: airgapped`
- `llm.local_base_url: ...`
- `audit.enabled: true`

#### 3) Запуск

```bash
cd backend
./target/debug/agent-cli --config config.yaml
```

#### 4) Проверка, что внешние вызовы отключены
При старте должна быть явная индикация Air-Gapped режима.
Также:
- инструменты сетевого доступа отключены (например, `fetch_url` не должен быть доступен)
- threat intel внешние источники не вызываются

