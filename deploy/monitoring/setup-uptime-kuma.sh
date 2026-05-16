#!/usr/bin/env bash
# Configure Uptime Kuma monitors (Socket.IO API — UK 1.23.x).
set -euo pipefail
ENV_FILE="${UK_ENV_FILE:-/etc/aegis/uptime-kuma.env}"
if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi
UK_URL="${UK_URL:-http://127.0.0.1:3001}"
UK_USER="${UK_USER:-aegis}"
UK_PASS="${UK_PASS:-}"
[[ -n "$UK_PASS" ]] || UK_PASS="$(openssl rand -hex 16)"

UK_PY="python3"
if ! python3 -c "import uptime_kuma_api" 2>/dev/null; then
  VENV="${UK_VENV:-/opt/aegis/monitoring/uk-venv}"
  if [[ ! -x "${VENV}/bin/python" ]]; then
    python3 -m venv "$VENV"
    "${VENV}/bin/pip" install -q 'uptime-kuma-api>=1.2.0,<1.3'
  fi
  UK_PY="${VENV}/bin/python"
fi

export UK_URL UK_USER UK_PASS
"${UK_PY}" <<'PY'
import os, sys

from uptime_kuma_api import MonitorType, UptimeKumaApi

base = os.environ["UK_URL"].rstrip("/")
user = os.environ["UK_USER"]
password = os.environ["UK_PASS"]

api = UptimeKumaApi(base, timeout=45)
try:
    api.login(user, password)
    print(f"login ok: {user}")
except Exception as login_err:
    err = str(login_err).lower()
    if "incorrect" in err or "password" in err:
        print(f"login failed: {login_err}", file=sys.stderr)
        sys.exit(1)
    api.setup(user, password)
    api.login(user, password)
    print(f"setup+login: {user}")

existing = {m.get("name") for m in (api.get_monitors() or [])}
monitors = [
    ("Primary /health", MonitorType.HTTP, {"url": "https://aegis-security.ru/health"}),
    ("Secondary /health", MonitorType.HTTP, {"url": "https://node2.aegis-security.ru/health"}),
    (
        "Primary federation :8443",
        MonitorType.PORT,
        {"hostname": "aegis-security.ru", "port": 8443},
    ),
    (
        "Secondary federation :8443",
        MonitorType.PORT,
        {"hostname": "node2.aegis-security.ru", "port": 8443},
    ),
]

for name, mtype, extra in monitors:
    if name in existing:
        print(f"skip {name}")
        continue
    kwargs = {"type": mtype, "name": name, "interval": 60, "maxretries": 2, "retryInterval": 60}
    kwargs.update(extra)
    mid = api.add_monitor(**kwargs)
    print(f"added {name} id={mid}")

api.disconnect()
PY

install -d -m 700 "$(dirname "$ENV_FILE")"
{
  echo "UK_USER=${UK_USER}"
  echo "UK_PASS=${UK_PASS}"
  echo "UK_URL=${UK_URL}"
} >"$ENV_FILE"
chmod 600 "$ENV_FILE"
echo "Credentials: ${ENV_FILE}"
