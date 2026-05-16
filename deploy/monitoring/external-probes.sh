#!/usr/bin/env bash
# External HTTP/TCP probes (fallback if Uptime Kuma UI not configured). Runs on primary.
set -euo pipefail
LOG_TAG=aegis-external-probe
fail=0
probe() { curl -sfS --max-time 15 "$1" >/dev/null || { logger -t "$LOG_TAG" "FAIL $1"; fail=1; }; }
probe https://aegis-security.ru/health
probe https://node2.aegis-security.ru/health
nc -z -w 5 aegis-security.ru 8443 || { logger -t "$LOG_TAG" "FAIL primary:8443"; fail=1; }
nc -z -w 5 node2.aegis-security.ru 8443 || { logger -t "$LOG_TAG" "FAIL node2:8443"; fail=1; }
exit $fail
