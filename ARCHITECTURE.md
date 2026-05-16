# Архитектура AEGIS

## Общая структура

```text
frontend/          # Next.js 14 + Tailwind + Framer Motion
├── app/
│   ├── dashboard/ # War Room интерфейс
│   └── page.tsx   # Лендинг
backend/           # Rust (Tokio + Axum + gRPC)
├── src/agent/     # Основная логика
│   ├── react_engine.rs
│   ├── critic_agent.rs
│   ├── fusion_engine.rs
│   ├── healing_orchestrator.rs
│   └── ...
src-tauri/         # Десктопное приложение
```

## Ключевые компоненты

### 1. Federation Layer
- P2P Discovery (Multicast)
- Дельта-синхронизация
- Merkle Root + Conflict Resolution
- **Два порта на каждой ноде:**
  - `:443` — публичный HTTPS (Let's Encrypt), health + dashboard
  - `:8443` — только `/federation/*`, обязательный client mTLS (отдельный Federation CA)
- Токен `X-AEGIS-Federation-Token` поверх mTLS между нодами

```mermaid
flowchart LR
  subgraph primary["aegis-security.ru"]
    P443["Nginx :443"]
    P8443["Nginx :8443\nssl_verify_client on"]
    AgentP["agent-cli :8080"]
    P443 --> AgentP
    P8443 --> AgentP
  end
  subgraph node2["node2.aegis-security.ru"]
    N443["Nginx :443"]
    N8443["Nginx :8443\nssl_verify_client on"]
    AgentN["agent-cli :8080"]
    N443 --> AgentN
    N8443 --> AgentN
  end
  P8443 <-->|"mTLS + federation token"| N8443
  P443 -.->|"GET /health"| N443
```

### 2. Self-Healing 2.0
- Healing Orchestrator + Formal Verification
- Частичная автономия (Low/Medium риски)
- Rollback Manager

### 3. Moving Target Defense + Advanced Deception
- Динамическая мутация fingerprint
- Автономное развёртывание honeypots
- Canary Tracking + авто-эскалация

### 4. Distributed Oracle (Raft) — production note
- **HA федерации в проде:** Merkle sync + mTLS `:8443` + health peers (см. Federation Layer).
- **Raft в UI/API:** ops-plane / индексация sync; при длительном простое heartbeats могут быть `stale` — это **не** автоматический failover кластера.
- Для пилота и SLA не обещайте «настоящий Raft consensus» без отдельного кворума ≥3 нод.

### 5. Verification
- AST Analysis
- Taint Tracking
- E2E-тесты

## Технологический стек

- **Язык:** Rust
- **Фронтенд:** Next.js 14 + TypeScript + Tailwind
- **База данных:** SQLite + Qdrant (векторная)
- **API:** Axum (HTTP) + Tonic (gRPC)
- **Десктоп:** Tauri
- **Деплой:** Vercel (фронтенд) + свой сервер (бэкенд)