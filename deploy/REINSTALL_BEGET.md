# Переустановка Ubuntu на Beget + чистый AEGIS

IP остаётся **178.236.16.101** — DNS менять не нужно.

## Шаг 1. Beget (панель)

1. **Виртуальные серверы** → **Decorous Theresa**.
2. **Переустановить** / **Сменить образ** → **Ubuntu 24.04 LTS** (не n8n).
3. Задайте **новый пароль root** → сохраните в менеджере паролей.
4. Дождитесь статуса «работает» (2–5 мин).

## Шаг 2. Проверка SSH с Mac

```bash
ssh root@178.236.16.101
```

Должен пустить по паролю. Если `Connection refused` — в веб-консоли Beget:

```bash
apt update && apt install -y openssh-server
systemctl enable --now ssh
```

## Шаг 3. Автоматическая установка AEGIS с Mac

```bash
cd /Users/ekaterinasacko/Desktop/AEGIS_FINAL
chmod +x deploy/bootstrap-from-mac.sh deploy/setup-vps.sh
export VPS_PASSWORD='ваш_новый_пароль_из_beget'
./deploy/bootstrap-from-mac.sh
```

Скрипт: nginx, firewall, Rust, сборка `agent-cli`, systemd, деплой `frontend/out`.

## Шаг 4. SSL (на VPS, один раз)

```bash
ssh root@178.236.16.101
certbot --nginx -d aegis-security.ru -d www.aegis-security.ru
```

Согласитесь с условиями, укажите email. Certbot обновит nginx (можно сверить с `deploy/nginx-aegis-site.conf`).

## Шаг 5. Секреты LLM (опционально)

```bash
nano /etc/aegis/agent.env
# AI_API_KEY=...  XAI_API_KEY=...
systemctl restart aegis-agent
```

## Шаг 6. Проверка

- https://aegis-security.ru — главная
- https://aegis-security.ru/dashboard/ — логин `root` / `1234` (тестовый ключ)
- На VPS: `systemctl status aegis-agent nginx`

## Обновить только фронт позже

```bash
cd frontend && npm run build
export VPS_PASSWORD='...'
./deploy_to_vps.sh
```

## Полезные команды на VPS

```bash
journalctl -u aegis-agent -f
curl -sS -X POST http://127.0.0.1:8080/api/login \
  -H 'Content-Type: application/json' \
  -d '{"api_key":"test-key-enterprise"}'
```
