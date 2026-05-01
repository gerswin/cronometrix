#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3001}"
USERNAME="${USERNAME:-e2e_admin}"
PASSWORD="${PASSWORD:-e2e-admin-pass}"
DURATION_SECONDS="${DURATION_SECONDS:-60}"
CONCURRENCY="${CONCURRENCY:-8}"
WRITE_RATIO="${WRITE_RATIO:-70}"
OUT_DIR="${OUT_DIR:-./load-test-results}"

command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required" >&2
  exit 1
}

mkdir -p "$OUT_DIR"

python3 - "$BASE_URL" "$USERNAME" "$PASSWORD" "$DURATION_SECONDS" "$CONCURRENCY" "$WRITE_RATIO" "$OUT_DIR" <<'PY'
import concurrent.futures as cf
import csv
import json
import os
import random
import statistics
import sys
import threading
import time
import urllib.error
import urllib.request
import uuid

base_url, username, password, duration_s, concurrency_s, write_ratio_s, out_dir = sys.argv[1:8]
duration_s = int(duration_s)
concurrency = int(concurrency_s)
write_ratio = int(write_ratio_s)
deadline = time.time() + duration_s

def request_json(method, path, payload=None, token=None):
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(base_url + path, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            body = resp.read().decode()
            return resp.getcode(), body, (time.perf_counter() - started) * 1000.0
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode(), (time.perf_counter() - started) * 1000.0
    except Exception as e:
        return 0, str(e), (time.perf_counter() - started) * 1000.0

login_code, login_body, _ = request_json(
    "POST",
    "/api/v1/auth/login",
    {"username": username, "password": password},
)
if login_code != 200:
    raise SystemExit(f"login failed: {login_code} {login_body}")
login_data = json.loads(login_body)
token = login_data.get("access_token") or login_data.get("token")
if not token:
    raise SystemExit("login response missing access_token")

dept_code, dept_body, _ = request_json("GET", "/api/v1/departments?limit=1", token=token)
if dept_code != 200:
    raise SystemExit(f"department lookup failed: {dept_code} {dept_body}")
dept_data = json.loads(dept_body)
items = dept_data.get("data") or []
if not items:
    raise SystemExit("no departments found; seed at least one department first")
department_id = items[0]["id"]

lock = threading.Lock()
stats = {
    "ok": 0,
    "fail": 0,
    "http500": 0,
    "reads": 0,
    "writes": 0,
}
latencies = []
latencies_read = []
latencies_write = []
sample_rows = []

def pick_operation():
    return "write" if random.randint(1, 100) <= write_ratio else "read"

def worker(worker_id: int):
    local = {
        "ok": 0,
        "fail": 0,
        "http500": 0,
        "reads": 0,
        "writes": 0,
    }
    local_latencies = []
    local_latencies_read = []
    local_latencies_write = []
    while time.time() < deadline:
        op = pick_operation()
        if op == "write":
            payload = {
                "employee_code": f"LT-{worker_id}-{uuid.uuid4().hex[:12]}",
                "name": "Load Test",
                "department_id": department_id,
                "position": "tester",
            }
            code, body, elapsed = request_json(
                "POST",
                "/api/v1/employees",
                payload,
                token=token,
            )
        else:
            code, body, elapsed = request_json(
                "GET",
                "/api/v1/employees?limit=1",
                token=token,
            )

        local_latencies.append(elapsed)
        if op == "read":
            local_latencies_read.append(elapsed)
            local["reads"] += 1
        else:
            local_latencies_write.append(elapsed)
            local["writes"] += 1

        if 200 <= code < 300:
            local["ok"] += 1
        else:
            local["fail"] += 1
            if code == 500:
                local["http500"] += 1
        if len(sample_rows) < 20:
            with lock:
                if len(sample_rows) < 20:
                    sample_rows.append({
                        "worker": worker_id,
                        "op": op,
                        "code": code,
                        "latency_ms": round(elapsed, 2),
                    })
    with lock:
        for k, v in local.items():
            stats[k] += v
        latencies.extend(local_latencies)
        latencies_read.extend(local_latencies_read)
        latencies_write.extend(local_latencies_write)

def percentile(values, pct):
    if not values:
        return None
    values = sorted(values)
    idx = int(round((pct / 100.0) * (len(values) - 1)))
    return round(values[max(0, min(idx, len(values) - 1))], 2)

started_at = time.time()
print(f"Running load test for {duration_s}s with concurrency={concurrency} write_ratio={write_ratio}%")
with cf.ThreadPoolExecutor(max_workers=concurrency) as ex:
    futures = [ex.submit(worker, i + 1) for i in range(concurrency)]
    for f in futures:
        f.result()
finished_at = time.time()

summary = {
    "base_url": base_url,
    "duration_seconds": duration_s,
    "concurrency": concurrency,
    "write_ratio": write_ratio,
    "started_at_epoch": round(started_at, 3),
    "finished_at_epoch": round(finished_at, 3),
    "ok": stats["ok"],
    "fail": stats["fail"],
    "http500": stats["http500"],
    "reads": stats["reads"],
    "writes": stats["writes"],
    "latency_ms": {
        "avg": round(statistics.mean(latencies), 2) if latencies else None,
        "p50": percentile(latencies, 50),
        "p95": percentile(latencies, 95),
        "p99": percentile(latencies, 99),
        "max": round(max(latencies), 2) if latencies else None,
    },
    "latency_read_ms": {
        "avg": round(statistics.mean(latencies_read), 2) if latencies_read else None,
        "p95": percentile(latencies_read, 95),
    },
    "latency_write_ms": {
        "avg": round(statistics.mean(latencies_write), 2) if latencies_write else None,
        "p95": percentile(latencies_write, 95),
    },
    "samples": sample_rows,
}

json_path = os.path.join(out_dir, f"load-test-{int(started_at)}.json")
csv_path = os.path.join(out_dir, f"load-test-{int(started_at)}.csv")
with open(json_path, "w", encoding="utf-8") as f:
    json.dump(summary, f, indent=2, sort_keys=True)

with open(csv_path, "w", newline="", encoding="utf-8") as f:
    writer = csv.writer(f)
    writer.writerow(["worker", "op", "code", "latency_ms"])
    for row in sample_rows:
        writer.writerow([row["worker"], row["op"], row["code"], row["latency_ms"]])

print(json.dumps(summary, indent=2, sort_keys=True))
print(f"json_report={json_path}")
print(f"csv_report={csv_path}")
PY
