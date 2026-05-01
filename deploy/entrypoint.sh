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
fi

exec /usr/local/bin/cronometrix
