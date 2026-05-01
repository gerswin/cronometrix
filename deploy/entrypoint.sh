#!/bin/sh
# Demo entrypoint: optional auto-seed on cold start, then exec the API binary.
# Auto-seed is enabled when CRONOMETRIX_AUTO_SEED=true. Seed binary is gated by
# CRONOMETRIX_E2E=true (already required for license bypass on demo).
# Idempotent: seed uses INSERT OR IGNORE; safe across container restarts.
set -e

if [ "${CRONOMETRIX_AUTO_SEED:-false}" = "true" ]; then
  echo "[entrypoint] CRONOMETRIX_AUTO_SEED=true — running seed_e2e"
  if /usr/local/bin/seed_e2e; then
    echo "[entrypoint] seed_e2e completed"
  else
    echo "[entrypoint] seed_e2e failed (non-fatal) — continuing to API boot"
  fi

  # Layer 2: seed daily_records covering every reportable scenario for the
  # current month + prior month. Idempotent: ON CONFLICT(employee_id,
  # anchor_date) DO UPDATE. Non-fatal — API boots even if this fails.
  DB_PATH="${CRONOMETRIX_DB_PATH:-/opt/cronometrix/data/cronometrix.db}"
  CUR_MONTH=$(date +%Y-%m)
  PREV_MONTH=$(date -d 'last month' +%Y-%m 2>/dev/null || date -v-1m +%Y-%m 2>/dev/null || echo "$CUR_MONTH")
  for M in "$PREV_MONTH" "$CUR_MONTH"; do
    echo "[entrypoint] running seed-reports-data.py --month $M"
    if python3 /usr/local/bin/seed-reports-data.py --db "$DB_PATH" --month "$M"; then
      echo "[entrypoint] seed-reports-data $M completed"
    else
      echo "[entrypoint] seed-reports-data $M failed (non-fatal)"
    fi
  done
fi

exec /usr/local/bin/cronometrix
