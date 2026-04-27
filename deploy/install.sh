#!/usr/bin/env bash
# Cronometrix one-command installer (DEPL-01).
# Usage:  curl -sSL https://install.cronometrix.com/install.sh | sudo bash
# Or:     sudo bash deploy/install.sh
#
# Idempotent: re-runs detect existing /opt/cronometrix/.env and skip secret
# regeneration so user sessions survive (Pitfall 7).

set -euo pipefail

# ---------------------------------------------------------------- config
INSTALL_DIR="${CRONOMETRIX_INSTALL_DIR:-/opt/cronometrix}"
DATA_DIR="${INSTALL_DIR}/data"
ENV_FILE="${INSTALL_DIR}/.env"
COMPOSE_FILE="${INSTALL_DIR}/docker-compose.yml"
TZ_DEFAULT="${CRONOMETRIX_TZ:-America/Caracas}"
IMAGE_REGISTRY="${CRONOMETRIX_IMAGE_REGISTRY:-ghcr.io/cronometrix}"
VERSION="${CRONOMETRIX_VERSION:-latest}"
# DO Functions URLs — operator MUST export these before running the installer.
# The activation curl call against the running API container would fail with
# AppError::BadGateway (ACTIVATION_UNREACHABLE) on every install if these are
# empty (Plan 01 service.activate_license guards against empty URL).
DO_FUNCTIONS_ACTIVATE_URL="${CRONOMETRIX_DO_ACTIVATE_URL:-}"
DO_FUNCTIONS_RENEW_URL="${CRONOMETRIX_DO_RENEW_URL:-}"

# ---------------------------------------------------------------- helpers
log() { printf '[install] %s\n' "$*" >&2; }
err() { printf '[install] ERROR: %s\n' "$*" >&2; exit 1; }

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        err "missing required command: $1"
    fi
}

require_root() {
    if [ "$(id -u)" -ne 0 ]; then
        err "must run as root (sudo)"
    fi
}

# ---------------------------------------------------------------- preflight
log "checking prerequisites..."
require_root
require_cmd docker
require_cmd openssl
require_cmd curl
require_cmd python3
if ! docker compose version >/dev/null 2>&1; then
    err "docker compose plugin not installed (need 'docker compose', not 'docker-compose')"
fi

# DO Functions URLs are required for license activation — fail fast if absent.
# Without these, the curl POST /setup/activate later in this script returns
# BadGateway (ACTIVATION_UNREACHABLE) and aborts the install with a confusing
# error from inside docker. Surface the real cause here.
if [ -z "$DO_FUNCTIONS_ACTIVATE_URL" ]; then
    err "DO_FUNCTIONS_ACTIVATE_URL is empty. Export CRONOMETRIX_DO_ACTIVATE_URL (and CRONOMETRIX_DO_RENEW_URL) with the URLs from 'doctl serverless functions get licenses/activate --url' before running this installer. See do-functions/README.md."
fi
if [ -z "$DO_FUNCTIONS_RENEW_URL" ]; then
    err "DO_FUNCTIONS_RENEW_URL is empty. Export CRONOMETRIX_DO_RENEW_URL with the URL from 'doctl serverless functions get licenses/renew --url' before running this installer. See do-functions/README.md."
fi

# ---------------------------------------------------------------- prompts
log "Cronometrix installer — 4 inputs required"
if [ -t 0 ]; then
    # Interactive
    read -r -p "License key (XXXX-XXXX-XXXX-XXXX): " LICENSE_KEY
    read -r -p "Client slug (becomes {slug}.cronometrix.com): " CLIENT_SLUG
    read -r -s -p "Admin password (min 8 chars): " ADMIN_PASSWORD
    echo
    read -r -p "Cloudflare tunnel token (from CF Zero Trust dashboard): " CF_TOKEN
else
    # Non-interactive — read from env vars (CI/CD)
    LICENSE_KEY="${CRONOMETRIX_LICENSE_KEY:?CRONOMETRIX_LICENSE_KEY required in non-interactive mode}"
    CLIENT_SLUG="${CRONOMETRIX_CLIENT_SLUG:?CRONOMETRIX_CLIENT_SLUG required in non-interactive mode}"
    ADMIN_PASSWORD="${CRONOMETRIX_ADMIN_PASSWORD:?CRONOMETRIX_ADMIN_PASSWORD required in non-interactive mode}"
    CF_TOKEN="${CRONOMETRIX_CF_TUNNEL_TOKEN:?CRONOMETRIX_CF_TUNNEL_TOKEN required in non-interactive mode}"
fi

# Validate license key shape (XXXX-XXXX-XXXX-XXXX, 19 chars)
if [ "${#LICENSE_KEY}" -ne 19 ] || ! echo "$LICENSE_KEY" | grep -qE '^[A-Za-z0-9]{4}-[A-Za-z0-9]{4}-[A-Za-z0-9]{4}-[A-Za-z0-9]{4}$'; then
    err "license key must be in XXXX-XXXX-XXXX-XXXX format"
fi
if [ "${#ADMIN_PASSWORD}" -lt 8 ]; then
    err "admin password must be at least 8 characters"
fi
if ! echo "$CLIENT_SLUG" | grep -qE '^[a-z0-9][a-z0-9-]{1,62}[a-z0-9]$'; then
    err "client slug must be 3-64 chars, lowercase alphanumeric + hyphens, no leading/trailing hyphen"
fi

# ---------------------------------------------------------------- directories
log "creating ${INSTALL_DIR} and ${DATA_DIR}"
mkdir -p "$INSTALL_DIR" "$DATA_DIR"
chmod 750 "$INSTALL_DIR" "$DATA_DIR"

# ---------------------------------------------------------------- secrets (idempotent)
if [ -f "$ENV_FILE" ]; then
    log "${ENV_FILE} exists — preserving existing JWT_SECRET / DEVICE_CREDS_KEY"
    # shellcheck disable=SC1090
    set -a; . "$ENV_FILE"; set +a
    : "${JWT_SECRET:=}"
    : "${DEVICE_CREDS_KEY:=}"
    if [ -z "$JWT_SECRET" ] || [ -z "$DEVICE_CREDS_KEY" ]; then
        err "existing $ENV_FILE missing JWT_SECRET or DEVICE_CREDS_KEY — back it up and remove to regenerate"
    fi
else
    log "generating fresh secrets via openssl rand"
    JWT_SECRET="$(openssl rand -hex 32)"
    DEVICE_CREDS_KEY="$(openssl rand -base64 32 | tr -d '\n')"
fi

# ---------------------------------------------------------------- write .env
log "writing ${ENV_FILE}"
umask 077  # 0600 perms — secrets file
cat > "$ENV_FILE" <<EOF
# Cronometrix runtime config — generated by install.sh
# Do NOT commit this file. chmod 600 enforced.
JWT_SECRET=${JWT_SECRET}
DEVICE_CREDS_KEY=${DEVICE_CREDS_KEY}
CLOUDFLARE_TUNNEL_TOKEN=${CF_TOKEN}
CLIENT_SLUG=${CLIENT_SLUG}
LICENSE_JWT_PATH=/opt/cronometrix/data/license.jwt
DO_FUNCTIONS_ACTIVATE_URL=${DO_FUNCTIONS_ACTIVATE_URL}
DO_FUNCTIONS_RENEW_URL=${DO_FUNCTIONS_RENEW_URL}
TZ=${TZ_DEFAULT}
CRONOMETRIX_DB_PATH=/opt/cronometrix/data/cronometrix.db
SERVER_HOST=0.0.0.0
SERVER_PORT=3001
VERSION=${VERSION}
EOF
chmod 600 "$ENV_FILE"
umask 022

# ---------------------------------------------------------------- compose file
if [ ! -f "$COMPOSE_FILE" ]; then
    log "writing ${COMPOSE_FILE}"
    # The installer ships a copy of docker-compose.yml alongside itself.
    # If invoked from the repo, copy from ./deploy/docker-compose.yml.
    # If invoked via curl|bash, the compose YAML is fetched from the same release.
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd)"
    if [ -f "${SCRIPT_DIR}/docker-compose.yml" ]; then
        cp "${SCRIPT_DIR}/docker-compose.yml" "$COMPOSE_FILE"
    else
        # Fetch from release artifact
        curl -fsSL "https://install.cronometrix.com/docker-compose.yml" -o "$COMPOSE_FILE"
    fi
fi

# ---------------------------------------------------------------- pull + start api first
log "pulling images: ${IMAGE_REGISTRY}/api:${VERSION}, ${IMAGE_REGISTRY}/web:${VERSION}, cloudflare/cloudflared:2026.3.0"
cd "$INSTALL_DIR"
docker compose pull

log "starting api (license activation requires it running)"
docker compose up -d api

# Wait for api healthcheck to pass (max 60s)
log "waiting for api health..."
for i in $(seq 1 30); do
    if curl -fsS http://127.0.0.1:3001/api/v1/health >/dev/null 2>&1; then
        log "api is healthy"
        break
    fi
    sleep 2
    if [ "$i" -eq 30 ]; then
        err "api did not become healthy within 60s — check 'docker compose logs api'"
    fi
done

# ---------------------------------------------------------------- activate license
log "activating license via the running api (api is the source of truth for fingerprint)"
ACTIVATE_RESP="$(
    curl -fsS -X POST http://127.0.0.1:3001/api/v1/setup/activate \
        -H "Content-Type: application/json" \
        -d "{\"license_key\":\"${LICENSE_KEY}\"}"
)" || err "license activation failed — see api logs ('docker compose logs api'); your hardware may already be bound to a different license"

if ! echo "$ACTIVATE_RESP" | python3 -c 'import sys,json; d=json.load(sys.stdin); sys.exit(0 if d.get("activated") else 1)'; then
    err "license activation returned unexpected response: $ACTIVATE_RESP"
fi
log "license activated"

# ---------------------------------------------------------------- start web + cloudflared
log "starting web and cloudflared"
docker compose up -d web cloudflared

# ---------------------------------------------------------------- create admin
log "creating admin user via /setup/init"
SETUP_RESP="$(
    curl -fsS -X POST http://127.0.0.1:3001/api/v1/setup/init \
        -H "Content-Type: application/json" \
        -d "{\"full_name\":\"Administrator\",\"username\":\"admin\",\"password\":\"${ADMIN_PASSWORD}\"}" \
    || true
)"
case "$SETUP_RESP" in
    *SETUP_ALREADY_COMPLETE*)
        log "admin already exists (setup re-run) — preserving existing credentials"
        ;;
    '')
        log "warning: /setup/init returned empty body; check api logs"
        ;;
    *)
        log "admin user created"
        ;;
esac

# ---------------------------------------------------------------- done
log "============================================================"
log "Cronometrix installation complete."
log "Local URL:    http://127.0.0.1:3000"
log "Public URL:   https://${CLIENT_SLUG}.cronometrix.com (once CF tunnel propagates)"
log "Admin user:   admin"
log "Manage:       cd ${INSTALL_DIR} && docker compose ps|logs|restart"
log "============================================================"
