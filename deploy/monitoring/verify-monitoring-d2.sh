#!/usr/bin/env bash
# D2 — verify external probes + Uptime Kuma monitors + Prometheus scrape.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fail=0

note() { echo "[d2] $*"; }
ok() { note "OK: $*"; }
bad() { note "FAIL: $*"; fail=1; }

note "external probes"
if bash "${ROOT}/external-probes.sh"; then
  ok "external-probes.sh"
else
  bad "external-probes.sh"
fi

note "docker stack"
for c in aegis-prometheus aegis-grafana aegis-uptime-kuma; do
  if docker ps --format '{{.Names}}' | grep -qx "$c"; then
    ok "container $c running"
  else
    bad "container $c not running"
  fi
done

note "prometheus scrape"
if curl -sf --max-time 10 'http://127.0.0.1:9090/-/healthy' >/dev/null; then
  up="$(curl -sf 'http://127.0.0.1:9090/api/v1/query?query=up' | python3 -c "
import sys,json
d=json.load(sys.stdin)
res=d.get('data',{}).get('result',[])
vals=[r['value'][1] for r in res if r.get('metric',{}).get('job')=='aegis-agent-primary']
print('1' if vals and vals[0]=='1' else '0')
" 2>/dev/null || echo 0)"
  [[ "$up" == "1" ]] && ok "prometheus up{aegis-agent-primary}=1" || bad "prometheus target down"
else
  bad "prometheus not healthy"
fi

note "uptime kuma monitors"
python3 <<'PY'
import sqlite3, subprocess, sys, time

vol = subprocess.check_output(
    ["docker", "volume", "inspect", "monitoring_uptime_kuma_data", "-f", "{{.Mountpoint}}"],
    text=True,
).strip()
conn = sqlite3.connect(vol + "/kuma.db")
cur = conn.cursor()
cur.execute("SELECT id, name, active FROM monitor")
mons = cur.fetchall()
if len(mons) < 4:
    print(f"FAIL: expected >=4 monitors, got {len(mons)}")
    sys.exit(1)
for mid, name, active in mons:
    if not active:
        print(f"FAIL: monitor {name} not active")
        sys.exit(1)
    cur.execute(
        "SELECT status FROM heartbeat WHERE monitor_id=? ORDER BY id DESC LIMIT 1",
        (mid,),
    )
    row = cur.fetchone()
    if not row:
        print(f"WARN: no heartbeat yet for {name} — waiting 65s")
        time.sleep(65)
        cur.execute(
            "SELECT status FROM heartbeat WHERE monitor_id=? ORDER BY id DESC LIMIT 1",
            (mid,),
        )
        row = cur.fetchone()
    if not row or row[0] != 1:
        print(f"FAIL: {name} status={row}")
        sys.exit(1)
    print(f"OK monitor {name} up")
PY
ok "kuma heartbeats"

if [[ "$fail" -ne 0 ]]; then
  echo "D2_VERIFY_FAIL"
  exit 1
fi
echo "D2_VERIFY_PASS"
