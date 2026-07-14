#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BACKEND_DIR="$REPO_ROOT/backend"
BASE_URL="${BASE_URL:-http://127.0.0.1:4001}"
SERVER_LOG="${SERVER_LOG:-/tmp/cronometrix-write-queue.log}"
DURATION_SECONDS="${DURATION_SECONDS:-60}"
OUT_DIR="${OUT_DIR:-$REPO_ROOT/.planning/phases/12-v1-0-release-stabilization/evidence/03-write-queue-load}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
VALIDATE_ONLY="${VALIDATE_ONLY:-false}"
ALLOW_SHORT_PROFILES="${ALLOW_SHORT_PROFILES:-false}"

if [[ "$DURATION_SECONDS" != "60" && "$ALLOW_SHORT_PROFILES" != "true" ]]; then
  echo "official write-queue profiles must run for exactly 60 seconds" >&2
  exit 2
fi
if [[ ! "$DURATION_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "DURATION_SECONDS must be a positive integer" >&2
  exit 2
fi
if [[ ! "$RUN_ID" =~ ^[A-Za-z0-9_-]+$ ]]; then
  echo "RUN_ID may contain only letters, digits, hyphens, and underscores" >&2
  exit 2
fi
if [[ "$BASE_URL" != "http://127.0.0.1:4001" ]]; then
  echo "isolated runner requires BASE_URL=http://127.0.0.1:4001" >&2
  exit 2
fi

PROFILES=(
  "c1-w100:1:100"
  "c32-r100:32:0"
  "c32-w100:32:100"
  "c32-w70:32:70"
)

if [[ "$VALIDATE_ONLY" == "true" ]]; then
  printf 'profile,concurrency,write_ratio,duration_seconds\n'
  for definition in "${PROFILES[@]}"; do
    IFS=: read -r profile concurrency write_ratio <<<"$definition"
    printf '%s,%s,%s,%s\n' "$profile" "$concurrency" "$write_ratio" "$DURATION_SECONDS"
  done
  exit 0
fi

for command in python3 docker curl; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "$command is required" >&2
    exit 1
  }
done

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/cronometrix-write-queue-load.XXXXXX")"
DB_PATH="$TMP_ROOT/cronometrix.db"
CONTAINER_SUFFIX="$(printf '%s' "$RUN_ID" | tr '[:upper:]' '[:lower:]')"
CONTAINER_NAME="cronometrix-12-03-load-$CONTAINER_SUFFIX"
IMAGE_TAG="cronometrix-12-03-load:$CONTAINER_SUFFIX"

cleanup() {
  local exit_code=$?
  trap - EXIT INT TERM
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
  docker image rm "$IMAGE_TAG" >/dev/null 2>&1 || true
  rm -rf "$TMP_ROOT"
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR"/c1-w100.{json,csv} \
  "$OUT_DIR"/c32-r100.{json,csv} \
  "$OUT_DIR"/c32-w100.{json,csv} \
  "$OUT_DIR"/c32-w70.{json,csv} \
  "$OUT_DIR"/profiles-summary.json \
  "$OUT_DIR"/reconciliation.csv \
  "$OUT_DIR"/run.txt \
  "$OUT_DIR"/server-log-scan.txt
: >"$SERVER_LOG"

printf 'Building isolated Linux release image...\n'
docker build --file "$REPO_ROOT/deploy/Dockerfile.api" --tag "$IMAGE_TAG" "$REPO_ROOT"
mkdir -p "$TMP_ROOT/data"
docker run --detach \
  --name "$CONTAINER_NAME" \
  --publish 127.0.0.1:4001:3001 \
  --volume "$TMP_ROOT/data:/opt/cronometrix/data" \
  --env CRONOMETRIX_E2E=true \
  --env CRONOMETRIX_LICENSE_BYPASS=true \
  --env CRONOMETRIX_DB_PATH=/opt/cronometrix/data/cronometrix.db \
  --env SERVER_HOST=0.0.0.0 \
  --env SERVER_PORT=3001 \
  --env JWT_SECRET=load-profile-secret-is-at-least-32-bytes \
  --env DEVICE_CREDS_KEY=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
  --env COOKIE_SECURE=false \
  --env TZ=America/Caracas \
  --env RUST_LOG=warn \
  --env LICENSE_JWT_PATH=/opt/cronometrix/data/license.jwt \
  --env CRONOMETRIX_LEAVES_ROOT=/opt/cronometrix/data/leaves \
  --env CRONOMETRIX_EVENTS_ROOT=/opt/cronometrix/data/events \
  --env ENROLLMENTS_DIR=/opt/cronometrix/data/enrollments \
  --env CRONOMETRIX_CAPTURES_TMP=/opt/cronometrix/data/captures-tmp \
  --env DATA_DIR=/opt/cronometrix/data \
  "$IMAGE_TAG" \
  sh -c '/usr/local/bin/seed_e2e >/dev/null && exec /usr/local/bin/cronometrix' \
  >/dev/null
for _ in $(seq 1 600); do
  if curl --fail --silent "$BASE_URL/api/v1/health" >/dev/null 2>&1; then
    break
  fi
  if [[ "$(docker inspect --format '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null || true)" != "true" ]]; then
    echo "backend exited before becoming healthy" >&2
    docker logs "$CONTAINER_NAME" >&2 || true
    exit 1
  fi
  sleep 0.1
done
curl --fail --silent "$BASE_URL/api/v1/health" >/dev/null

for definition in "${PROFILES[@]}"; do
  IFS=: read -r profile concurrency write_ratio <<<"$definition"
  PROFILE_NAME="$profile" \
  CONCURRENCY="$concurrency" \
  WRITE_RATIO="$write_ratio" \
  DURATION_SECONDS="$DURATION_SECONDS" \
  RUN_ID="$RUN_ID" \
  BASE_URL="$BASE_URL" \
  OUT_DIR="$OUT_DIR" \
    bash "$BACKEND_DIR/scripts/load_test.sh"
done

if [[ "$(docker inspect --format '{{.State.Running}}' "$CONTAINER_NAME")" != "true" ]]; then
  docker logs "$CONTAINER_NAME" >"$SERVER_LOG" 2>&1 || true
  echo "backend exited before graceful shutdown" >&2
  exit 1
fi
docker stop --signal TERM --time 60 "$CONTAINER_NAME" >/dev/null
BACKEND_EXIT="$(docker inspect --format '{{.State.ExitCode}}' "$CONTAINER_NAME")"
docker logs "$CONTAINER_NAME" >"$SERVER_LOG" 2>&1
if [[ "$BACKEND_EXIT" -ne 0 ]]; then
  echo "backend exited with status $BACKEND_EXIT after SIGTERM" >&2
  exit 1
fi

DB_PATH="$TMP_ROOT/data/cronometrix.db"

python3 - "$OUT_DIR" "$DB_PATH" "$SERVER_LOG" "$RUN_ID" "$DURATION_SECONDS" <<'PY'
import csv
import json
import pathlib
import sqlite3
import sys

out_dir = pathlib.Path(sys.argv[1])
db_path = pathlib.Path(sys.argv[2])
server_log = pathlib.Path(sys.argv[3])
run_id = sys.argv[4]
duration = int(sys.argv[5])
profiles = ["c1-w100", "c32-r100", "c32-w100", "c32-w70"]
summaries = []
failures = []
reconciliation = []

with sqlite3.connect(db_path) as connection:
    for profile in profiles:
        report_path = out_dir / f"{profile}.json"
        with report_path.open(encoding="utf-8") as report:
            summary = json.load(report)
        summaries.append(summary)
        for metric in ("http500", "http503", "other_failures", "db_write_queue_busy"):
            if summary[metric] != 0:
                failures.append(f"{profile}: {metric}={summary[metric]}")
        if summary["http2xx"] != summary["reads"] + summary["writes"]:
            failures.append(f"{profile}: non-2xx request count detected")
        prefix = summary["employee_code_prefix"]
        persisted = connection.execute(
            "SELECT COUNT(*) FROM employees WHERE employee_code LIKE ? ESCAPE '\\'",
            (prefix.replace("%", "\\%").replace("_", "\\_") + "%",),
        ).fetchone()[0]
        accepted = summary["write_2xx"]
        reconciliation.append((profile, prefix, accepted, persisted))
        if accepted != persisted:
            failures.append(
                f"{profile}: accepted writes {accepted} != persisted rows {persisted}"
            )

log_text = server_log.read_text(encoding="utf-8", errors="replace")
locked_lines = [line for line in log_text.splitlines() if "database is locked" in line.lower()]
if locked_lines:
    failures.append(f"server log contains {len(locked_lines)} database-is-locked lines")

with (out_dir / "reconciliation.csv").open("w", newline="", encoding="utf-8") as report:
    writer = csv.writer(report, lineterminator="\n")
    writer.writerow(["profile", "employee_code_prefix", "accepted_write_2xx", "persisted_rows"])
    writer.writerows(reconciliation)

aggregate = {
    "run_id": run_id,
    "execution_platform": "linux/arm64 Docker",
    "duration_seconds_per_profile": duration,
    "graceful_shutdown_exit": 0,
    "database_locked_occurrences": len(locked_lines),
    "profiles": summaries,
    "reconciliation": [
        {
            "profile": profile,
            "accepted_write_2xx": accepted,
            "persisted_rows": persisted,
            "matched": accepted == persisted,
        }
        for profile, _prefix, accepted, persisted in reconciliation
    ],
    "verdict": "PASS" if not failures else "FAIL",
}
with (out_dir / "profiles-summary.json").open("w", encoding="utf-8") as report:
    json.dump(aggregate, report, indent=2, sort_keys=True)
    report.write("\n")
with (out_dir / "server-log-scan.txt").open("w", encoding="utf-8") as report:
    report.write(f"database_is_locked_occurrences={len(locked_lines)}\n")
    report.write("backend_sigterm_exit=0\n")
with (out_dir / "run.txt").open("w", encoding="utf-8") as report:
    report.write(f"run_id={run_id}\n")
    report.write(f"duration_seconds_per_profile={duration}\n")
    report.write("profiles=c1-w100,c32-r100,c32-w100,c32-w70\n")
    report.write("execution_platform=linux/arm64 Docker\n")
    report.write(f"verdict={'PASS' if not failures else 'FAIL'}\n")

if failures:
    raise SystemExit("; ".join(failures))
PY

printf 'Load profiles passed; evidence: %s\n' "$OUT_DIR"
