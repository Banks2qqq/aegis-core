#!/usr/bin/env bash
# Запускать ТОЛЬКО на MacBook (двойной клик или: bash УСТАНОВКА-НА-СЕРВЕР.sh)
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo ""
  echo "  СТОП: вы на СЕРВЕРЕ (Linux), не на Mac."
  echo "  Наберите:  exit"
  echo "  Потом снова запустите этот скрипт на Mac."
  echo ""
  exit 1
fi

cd "$(dirname "$0")"
HOST="178.236.16.101"

echo ""
echo "  Шаг 1/3 — проверка входа на сервер без пароля в скрипте"
echo "  (если спросит пароль — введите тот, что задали в Beget при переустановке Ubuntu)"
echo ""
if ! ssh -o BatchMode=yes -o ConnectTimeout=8 "root@${HOST}" "echo OK" 2>/dev/null; then
  echo "  Ключ ещё не настроен. Сейчас один раз спросят пароль для копирования ключа."
  if [[ ! -f "$HOME/.ssh/id_ed25519.pub" ]]; then
    ssh-keygen -t ed25519 -N "" -f "$HOME/.ssh/id_ed25519"
  fi
  ssh-copy-id -o StrictHostKeyChecking=no "root@${HOST}"
fi

echo ""
echo "  Шаг 2/3 — установка AEGIS на VPS (10–20 мин, не закрывать окно)"
echo ""
./deploy/bootstrap-from-mac.sh

echo ""
echo "  Шаг 3/3 — SSL (в этом же окне, спросит email для Let's Encrypt):"
echo ""
ssh -t "root@${HOST}" "certbot --nginx -d aegis-security.ru -d www.aegis-security.ru"

echo ""
echo "  Готово: https://aegis-security.ru"
echo "  Дашборд: https://aegis-security.ru/dashboard/  логин root  пароль 1234"
echo ""
