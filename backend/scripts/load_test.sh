#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:4001}"
USERNAME="${USERNAME:-e2e_admin}"
PASSWORD="${PASSWORD:-e2e-admin-pass}"
DURATION_SECONDS="${DURATION_SECONDS:-60}"
CONCURRENCY="${CONCURRENCY:-8}"
WRITE_RATIO="${WRITE_RATIO:-70}"
OUT_DIR="${OUT_DIR:-./load-test-results}"
PROFILE_NAME="${PROFILE_NAME:-custom}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"

command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required" >&2
  exit 1
}

mkdir -p "$OUT_DIR"

python3 - "$BASE_URL" "$USERNAME" "$PASSWORD" "$DURATION_SECONDS" \
  "$CONCURRENCY" "$WRITE_RATIO" "$OUT_DIR" "$PROFILE_NAME" "$RUN_ID" <<'PY'
import concurrent.futures as cf
import csv
import json
import os
import random
import sys
import threading
import time
import urllib.error
import urllib.request
import uuid

(
    base_url,
    username,
    password,
    duration_s,
    concurrency_s,
    write_ratio_s,
    out_dir,
    profile_name,
    run_id,
) = sys.argv[1:10]

try:
    duration_s = int(duration_s)
    concurrency = int(concurrency_s)
    write_ratio = int(write_ratio_s)
except ValueError as error:
    raise SystemExit(f"duration, concurrency, and write ratio must be integers: {error}")
if duration_s <= 0:
    raise SystemExit("duration must be greater than zero")
if concurrency <= 0:
    raise SystemExit("concurrency must be greater than zero")
if not 0 <= write_ratio <= 100:
    raise SystemExit("write ratio must be between 0 and 100")
if not profile_name or any(char not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_" for char in profile_name):
    raise SystemExit("profile name may contain only letters, digits, hyphens, and underscores")
if not run_id or any(char not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_" for char in run_id):
    raise SystemExit("run ID may contain only letters, digits, hyphens, and underscores")

# Kept below the employee-code 50-character validation limit even with the UUID suffix.
employee_code_prefix = f"LT-{run_id[:12]}-{profile_name[:12]}-"
deadline = time.monotonic() + duration_s


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
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode(), (time.perf_counter() - started) * 1000.0
    except Exception as error:
        return 0, str(error), (time.perf_counter() - started) * 1000.0


login_code, login_body, _ = request_json(
    "POST",
    "/api/v1/auth/login",
    {"username": username, "password": password},
)
if login_code != 200:
    raise SystemExit(f"login failed: HTTP {login_code}")
login_data = json.loads(login_body)
token = login_data.get("access_token") or login_data.get("token")
if not token:
    raise SystemExit("login response missing access_token")

dept_code, dept_body, _ = request_json("GET", "/api/v1/departments?limit=1", token=token)
if dept_code != 200:
    raise SystemExit(f"department lookup failed: HTTP {dept_code}")
dept_data = json.loads(dept_body)
items = dept_data.get("data") or []
if not items:
    raise SystemExit("no departments found; seed at least one department first")
department_id = items[0]["id"]

lock = threading.Lock()
stats = {
    "http2xx": 0,
    "http500": 0,
    "http503": 0,
    "db_write_queue_busy": 0,
    "other_failures": 0,
    "reads": 0,
    "writes": 0,
    "read_2xx": 0,
    "write_2xx": 0,
}
latencies = []
latencies_read = []
latencies_write = []
sample_rows = []


def pick_operation():
    return "write" if random.randint(1, 100) <= write_ratio else "read"


def is_queue_busy(body):
    try:
        payload = json.loads(body)
    except (json.JSONDecodeError, TypeError):
        return False
    return payload.get("code") == "DB_WRITE_QUEUE_BUSY"


def worker(worker_id):
    local = {key: 0 for key in stats}
    local_latencies = []
    local_latencies_read = []
    local_latencies_write = []
    local_samples = []
    while time.monotonic() < deadline:
        op = pick_operation()
        if op == "write":
            payload = {
                "employee_code": f"{employee_code_prefix}{uuid.uuid4().hex[:12]}",
                "name": "Load Test",
                "department_id": department_id,
                "position": "tester",
            }
            code, body, elapsed = request_json(
                "POST", "/api/v1/employees", payload, token=token
            )
        else:
            code, body, elapsed = request_json(
                "GET", "/api/v1/employees?limit=1", token=token
            )

        local_latencies.append(elapsed)
        local["reads" if op == "read" else "writes"] += 1
        if op == "read":
            local_latencies_read.append(elapsed)
        else:
            local_latencies_write.append(elapsed)

        if 200 <= code < 300:
            local["http2xx"] += 1
            local["read_2xx" if op == "read" else "write_2xx"] += 1
        elif code == 500:
            local["http500"] += 1
        elif code == 503:
            local["http503"] += 1
            if is_queue_busy(body):
                local["db_write_queue_busy"] += 1
        else:
            local["other_failures"] += 1

        if len(local_samples) < 2:
            local_samples.append(
                {
                    "worker": worker_id,
                    "op": op,
                    "code": code,
                    "latency_ms": round(elapsed, 2),
                }
            )

    with lock:
        for key, value in local.items():
            stats[key] += value
        latencies.extend(local_latencies)
        latencies_read.extend(local_latencies_read)
        latencies_write.extend(local_latencies_write)
        remaining = max(0, 20 - len(sample_rows))
        sample_rows.extend(local_samples[:remaining])


def percentile(values, pct):
    if not values:
        return None
    ordered = sorted(values)
    index = int(round((pct / 100.0) * (len(ordered) - 1)))
    return round(ordered[max(0, min(index, len(ordered) - 1))], 2)


def latency_summary(values):
    return {
        "p50": percentile(values, 50),
        "p95": percentile(values, 95),
        "p99": percentile(values, 99),
    }


started_at = time.time()
print(
    f"Running {profile_name} for {duration_s}s with "
    f"concurrency={concurrency} write_ratio={write_ratio}%"
)
with cf.ThreadPoolExecutor(max_workers=concurrency) as executor:
    futures = [executor.submit(worker, index + 1) for index in range(concurrency)]
    for future in futures:
        future.result()
finished_at = time.time()

summary = {
    "profile": profile_name,
    "run_id": run_id,
    "base_url": base_url,
    "duration_seconds": duration_s,
    "concurrency": concurrency,
    "write_ratio": write_ratio,
    "employee_code_prefix": employee_code_prefix,
    "started_at_epoch": round(started_at, 3),
    "finished_at_epoch": round(finished_at, 3),
    **stats,
    "latency_ms": {
        "all": latency_summary(latencies),
        "read": latency_summary(latencies_read),
        "write": latency_summary(latencies_write),
    },
    "samples": sample_rows,
}

json_path = os.path.join(out_dir, f"{profile_name}.json")
csv_path = os.path.join(out_dir, f"{profile_name}.csv")
with open(json_path, "w", encoding="utf-8") as report:
    json.dump(summary, report, indent=2, sort_keys=True)
    report.write("\n")

with open(csv_path, "w", newline="", encoding="utf-8") as report:
    writer = csv.writer(report, lineterminator="\n")
    writer.writerow(["worker", "op", "code", "latency_ms"])
    for row in sample_rows:
        writer.writerow([row["worker"], row["op"], row["code"], row["latency_ms"]])

print(json.dumps(summary, indent=2, sort_keys=True))
print(f"json_report={json_path}")
print(f"csv_report={csv_path}")
PY
