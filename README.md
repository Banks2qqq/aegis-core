# AEGIS — Автономный Цифровой Иммунитет

**Версия:** 0.9.0  
**Статус:** Готов к пилоту

AEGIS — первая в мире **автономная иммунная система** для цифровой инфраструктуры. Мы защищаем нечеловеческие идентификаторы (NHI): API-ключи, сервисные аккаунты, контейнеры и AI-агентов.

## Ключевые возможности

- **Federation Layer** — распределённая P2P-сеть с дельта-синхронизацией
- **Self-Healing 2.0** — частичная автономия + автоматическое восстановление
- **Moving Target Defense** — постоянная мутация поверхности атаки
- **Advanced Deception** — автономное развёртывание honeypots
- **Raft 2.0** — реальный распределённый консенсус с log replication
- **AST + Taint Verification** — формальная верификация кода
- **HSM / Vault** — безопасное хранение всех секретов

## Быстрый старт

### 1. Запуск бэкенда

```bash
cd backend
cargo run --bin agent-cli
```

### 2. Запуск дашборда (локально)

```bash
cd frontend
npm run dev
```
Откройте http://localhost:3000

### 3. Запуск десктопного приложения (Tauri)

```bash
cd src-tauri
cargo tauri dev
```

## Архитектура

AEGIS построен по принципам Zero-Trust на всех уровнях:

- Phase 1: Zero-Trust Foundation
- Phase 2: Self-Healing & Defense
- Phase 3: Federation & Deception
- Phase 4: Hardened Verification
- Phase 5: Autonomous Evolution

Подробная архитектура: [ARCHITECTURE.md](ARCHITECTURE.md)

## Контакты

- **Сайт:** [aegis-security.ru](https://aegis-security.ru)
- **Email:** maksim@aegis-security.ru

---
*AEGIS. Ваша инфраструктура больше не жертва. Она — хищник.*