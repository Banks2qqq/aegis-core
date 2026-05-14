## Архитектура (Pilot)

### Компоненты
- **agent-cli (Rust / Axum)**: основной рантайм агента, CLI-команды, HTTP/WebSocket API.
- **Config Layer**: YAML + env `AEGIS_` + CLI, явный Air-Gapped режим.
- **Local LLM Client**: Ollama/vLLM (OpenAI-compatible `/v1/chat/completions`).
- **ReAct++ Engine**: цикл рассуждения/действия, инструменты, MCTS-ранжирование.
- **CriticAgent**: оценка риска/полезности, Kill Switch, эскалация при \(risk>0.8\).
- **ToolRegistry**: контролируемый набор инструментов; в Air-Gapped сетевые инструменты отключены.
- **Fusion Engine**: корреляция событий и IOC (стриминг).
- **Threat Hunter**: сбор находок (в Air-Gapped — offline findings).
- **Immutable Audit Trail**: append-only `audit.log` + hash-chain.

### Поток данных (упрощённо)

```mermaid
flowchart LR
  CLI[agent-cli CLI] -->|commands| RE[ReAct++ Engine]
  RE --> CA[CriticAgent]
  RE --> TR[ToolRegistry]
  TR --> KB[KnowledgeBase]
  TR --> ISO[Isolation/Sandbox]
  TH[ThreatHunter] --> FE[Fusion Engine]
  RE --> FE
  FE --> API[HTTP/WebSocket API]
  CLI --> API
  CA -->|ESCALATE/BLOCK| HITL[Human-in-the-Loop]
  HITL --> CLI
  CLI --> AUD[AuditTrail audit.log]
  RE --> AUD
  CA --> AUD
  TH --> AUD
```

### Границы доверия (Zero-Trust)
- **LLM** рассматривается как недоверенный компонент (потенциально вредоносный вывод).
- Все “опасные” решения должны:
  - проходить оценку Critic,
  - при высоком риске требовать подтверждение человека,
  - фиксироваться в неизменяемом аудите.

