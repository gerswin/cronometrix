#!/usr/bin/env bash
# Cronometrix Dokku setup — run ON the Dokku host (192.168.0.44) as root or sudo.
# Supports BOTH dokkuised installs:
#   - Native: `dokku` is a host binary in PATH.
#   - Docker: Dokku runs inside a container (default name: `dokku`).
# Idempotent: safe to re-run.
set -euo pipefail

export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/snap/bin:$PATH"

# ---------- Detect dokku transport ----------
DOKKU_CONTAINER="${DOKKU_CONTAINER:-dokku}"
if command -v dokku >/dev/null 2>&1; then
  DOKKU_MODE="native"
  dk()   { dokku "$@"; }
  dksh() { bash -c "$1"; }
elif command -v docker >/dev/null 2>&1 && docker inspect "$DOKKU_CONTAINER" >/dev/null 2>&1; then
  DOKKU_MODE="docker:${DOKKU_CONTAINER}"
  dk()   { docker exec -i "$DOKKU_CONTAINER" dokku "$@"; }
  dksh() { docker exec -i "$DOKKU_CONTAINER" bash -c "$1"; }
else
  echo "FATAL: no dokku found." >&2
  echo "       Looked for: native binary in PATH, docker container '$DOKKU_CONTAINER'" >&2
  echo "       Verify with: which dokku  OR  docker ps --filter name=dokku" >&2
  exit 1
fi
echo "[setup] mode: $DOKKU_MODE"
dk version 2>/dev/null | head -1 || true

API_APP="${API_APP:-cronometrix-api}"
WEB_APP="${WEB_APP:-cronometrix-web}"
API_DOMAIN="${API_DOMAIN:-demo-api.cronometrix.app}"
WEB_DOMAIN="${WEB_DOMAIN:-app-demo.cronometrix.app}"
API_URL="https://${API_DOMAIN}"
WEB_URL="https://${WEB_DOMAIN}"
LE_EMAIL="${LE_EMAIL:-g3rswin@gmail.com}"

: "${JWT_SECRET:?export JWT_SECRET — used by backend for HS256 token signing}"
: "${TURSO_DATABASE_URL:?export TURSO_DATABASE_URL}"
: "${TURSO_AUTH_TOKEN:?export TURSO_AUTH_TOKEN}"

DEMO_MODE="${DEMO_MODE:-true}"

# ---------- Apps ----------
dk apps:create "$API_APP" 2>/dev/null || true
dk apps:create "$WEB_APP" 2>/dev/null || true

# ---------- Builder: each app -> its own Dockerfile inside the monorepo ----------
dk builder-dockerfile:set "$API_APP" dockerfile-path deploy/Dockerfile.api
dk builder-dockerfile:set "$WEB_APP" dockerfile-path deploy/Dockerfile.web

# ---------- Persistent storage for SQLite ----------
# In docker-mode the /var/lib/dokku tree lives INSIDE the dokku container.
# Use `dokku storage:ensure-directory` so it works in both modes.
dk storage:ensure-directory "$API_APP" 2>/dev/null || true
dk storage:mount "$API_APP" "/var/lib/dokku/data/storage/${API_APP}:/opt/cronometrix/data" 2>/dev/null || true

# ---------- API config ----------
dk config:set --no-restart "$API_APP" \
  RUST_LOG=info \
  TZ=America/Caracas \
  SERVER_HOST=0.0.0.0 \
  SERVER_PORT=3001 \
  CRONOMETRIX_DB_PATH=/opt/cronometrix/data/cronometrix.db \
  LICENSE_JWT_PATH=/opt/cronometrix/data/license.jwt \
  CORS_ALLOWED_ORIGINS="$WEB_URL" \
  COOKIE_SECURE=true \
  JWT_SECRET="$JWT_SECRET" \
  TURSO_DATABASE_URL="$TURSO_DATABASE_URL" \
  TURSO_AUTH_TOKEN="$TURSO_AUTH_TOKEN"

if [ "$DEMO_MODE" = "true" ]; then
  dk config:set --no-restart "$API_APP" \
    CRONOMETRIX_E2E=true \
    CRONOMETRIX_LICENSE_BYPASS=true
  echo "[demo] license bypass active. Will block /api/v1/__test_reset at nginx."
fi

# ---------- WEB config (NEXT_PUBLIC_* inlined at build) ----------
# Next.js bundles NEXT_PUBLIC_* AT BUILD TIME, so config:set alone is not enough —
# we must also pass --build-arg so the Dockerfile's ARG picks up the production URL.
dk config:set --no-restart "$WEB_APP" \
  NEXT_PUBLIC_API_URL="$API_URL" \
  NODE_ENV=production \
  NEXT_TELEMETRY_DISABLED=1
# Add the build-arg. docker-options:add is not idempotent and stacks duplicates
# on re-run, but duplicate --build-arg with the same value is harmless. Best-effort
# cleanup of any prior entry — failure is non-fatal (`|| true`) because a fresh
# app has no options to enumerate.
{ dk docker-options:remove "$WEB_APP" build "--build-arg NEXT_PUBLIC_API_URL=$API_URL" 2>/dev/null || true; }
dk docker-options:add "$WEB_APP" build "--build-arg NEXT_PUBLIC_API_URL=$API_URL"

# ---------- Domains ----------
dk domains:set "$API_APP" "$API_DOMAIN"
dk domains:set "$WEB_APP" "$WEB_DOMAIN"

# ---------- Port mapping ----------
dk ports:set "$API_APP" http:80:3001 https:443:3001
dk ports:set "$WEB_APP" http:80:3000 https:443:3000

# ---------- Let's Encrypt config (enable separately after DNS is live) ----------
dk letsencrypt:set "$API_APP" email "$LE_EMAIL" 2>/dev/null || \
  echo "WARN: letsencrypt plugin not installed. Install with: dokku plugin:install https://github.com/dokku/dokku-letsencrypt.git"
dk letsencrypt:set "$WEB_APP" email "$LE_EMAIL" 2>/dev/null || true

# ---------- Healthcheck (api only) ----------
dk checks:set "$API_APP" web /api/v1/health 2>/dev/null || true

# ---------- Block destructive demo route at nginx layer ----------
# CRONOMETRIX_E2E=true exposes POST /api/v1/__test_reset which WIPES tenant data
# with no auth. Block at nginx so the public internet cannot reach it.
# nginx config lives inside the dokku container in docker-mode.
NGINX_REL="/home/dokku/${API_APP}/nginx.conf.d"
dksh "mkdir -p '$NGINX_REL' && cat > '${NGINX_REL}/block-test-reset.conf' <<'NGX'
location = /api/v1/__test_reset {
    return 404;
}
NGX
chown -R dokku:dokku '$NGINX_REL' 2>/dev/null || true"
dk nginx:build-config "$API_APP" 2>/dev/null || true

cat <<EOF

[OK] Dokku apps configured (mode: ${DOKKU_MODE}).

NEXT STEPS:

  1. Git remotes (from your laptop):
     git remote add dokku-api dokku@${SSH_HOST:-<host>}:${API_APP}
     git remote add dokku-web dokku@${SSH_HOST:-<host>}:${WEB_APP}

  2. Push:
     git push dokku-api main:main
     git push dokku-web main:main

  3. TLS (after DNS A records resolve to the host's public IP):
     dokku letsencrypt:enable ${API_APP}
     dokku letsencrypt:enable ${WEB_APP}

  4. Smoke:
     curl -fsS ${API_URL}/api/v1/health
     curl -i  -X POST ${API_URL}/api/v1/__test_reset   # expect 404

DEMO_MODE active:
  - License gate bypassed (no license.jwt needed)
  - /__test_reset registered inside pod but BLOCKED at nginx (404)
  - DO NOT reuse this config for a real customer deployment
EOF
