#!/usr/bin/env bash
# Run ON secondary VPS: HTTPS with self-signed cert (for federation over IP).
# Usage: install-nginx-selfsigned.sh [server_name]
set -euo pipefail
SERVER_NAME="${1:-93.189.230.72}"
SSL_DIR="/etc/nginx/ssl/aegis-selfsigned"
mkdir -p "$SSL_DIR"

if [[ ! -f "$SSL_DIR/server.crt" ]]; then
  openssl req -x509 -nodes -days 825 -newkey rsa:2048 \
    -keyout "$SSL_DIR/server.key" \
    -out "$SSL_DIR/server.crt" \
    -subj "/CN=${SERVER_NAME}"
  chmod 600 "$SSL_DIR/server.key"
fi

cat > /etc/nginx/sites-available/aegis <<NGINX
server {
    listen 80;
    server_name ${SERVER_NAME};
    return 301 https://\$host\$request_uri;
}

server {
    listen 443 ssl;
    server_name ${SERVER_NAME};

    ssl_certificate ${SSL_DIR}/server.crt;
    ssl_certificate_key ${SSL_DIR}/server.key;

    root /var/www/aegis/html;
    index index.html;

    location /api/ {
        proxy_pass http://127.0.0.1:8080/api/;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_read_timeout 300s;
    }

    location /ws {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_read_timeout 86400;
    }

    location = /health {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
    }

    location = /api/health {
        proxy_pass http://127.0.0.1:8080/health;
        proxy_set_header Host \$host;
    }

    location /federation/ {
        proxy_pass http://127.0.0.1:8080/federation/;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    location / {
        try_files \$uri \$uri.html \$uri/ /index.html;
    }
}
NGINX

rm -f /etc/nginx/sites-enabled/default
ln -sf /etc/nginx/sites-available/aegis /etc/nginx/sites-enabled/aegis
nginx -t
systemctl reload nginx
echo "[nginx] Self-signed HTTPS for ${SERVER_NAME}"
