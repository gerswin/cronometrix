#!/usr/bin/env bash
# Cronometrix Dokku one-shot deploy — run from your laptop in the repo root.
# Usage:
#   cp deploy/dokku/deploy.env.example deploy/dokku/deploy.env
#   # edit deploy.env with your secrets
#   bash deploy/dokku/deploy.sh
#
# Steps performed:
#   1. Load secrets from deploy/dokku/deploy.env (gitignored).
#   2. scp setup.sh to host, run via ssh+sudo with secrets in env.
#   3. Add git remotes (idempotent).
#   4. git push api + web.
#   5. Enable Let's Encrypt (skipped if SKIP_LE=true).
#   6. Smoke test endpoints.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

ENV_FILE="${ENV_FILE:-deploy/dokku/deploy.env}"
if [ ! -f "$ENV_FILE" ]; then
  echo "FATAL: $ENV_FILE not found." >&2
  echo "       cp deploy/dokku/deploy.env.example $ENV_FILE  &&  edit it." >&2
  exit 1
fi
# shellcheck disable=SC1090
set -a; . "$ENV_FILE"; set +a

: "${SSH_USER:?set SSH_USER in $ENV_FILE}"
: "${SSH_HOST:?set SSH_HOST in $ENV_FILE}"
: "${API_APP:?set API_APP in $ENV_FILE}"
: "${WEB_APP:?set WEB_APP in $ENV_FILE}"
: "${API_DOMAIN:?set API_DOMAIN in $ENV_FILE}"
: "${WEB_DOMAIN:?set WEB_DOMAIN in $ENV_FILE}"
: "${JWT_SECRET:?set JWT_SECRET in $ENV_FILE}"
: "${TURSO_DATABASE_URL:?set TURSO_DATABASE_URL in $ENV_FILE}"
: "${TURSO_AUTH_TOKEN:?set TURSO_AUTH_TOKEN in $ENV_FILE}"
: "${LE_EMAIL:?set LE_EMAIL in $ENV_FILE}"
SKIP_LE="${SKIP_LE:-false}"
GIT_BRANCH="${GIT_BRANCH:-main}"
DOKKU_SSH_PORT="${DOKKU_SSH_PORT:-22}"   # 3022 when Dokku runs in a Docker container

SSH="ssh -o StrictHostKeyChecking=accept-new ${SSH_USER}@${SSH_HOST}"
SCP="scp -o StrictHostKeyChecking=accept-new"

echo "==> [1/6] Uploading setup.sh to ${SSH_HOST}"
$SCP deploy/dokku/setup.sh "${SSH_USER}@${SSH_HOST}:/tmp/cronometrix-setup.sh"

echo "==> [2/6] Running setup.sh on host (sudo)"
$SSH -t "sudo -E env \
  API_APP='${API_APP}' \
  WEB_APP='${WEB_APP}' \
  API_DOMAIN='${API_DOMAIN}' \
  WEB_DOMAIN='${WEB_DOMAIN}' \
  LE_EMAIL='${LE_EMAIL}' \
  JWT_SECRET='${JWT_SECRET}' \
  TURSO_DATABASE_URL='${TURSO_DATABASE_URL}' \
  TURSO_AUTH_TOKEN='${TURSO_AUTH_TOKEN}' \
  DEMO_MODE='${DEMO_MODE:-true}' \
  bash /tmp/cronometrix-setup.sh"

echo "==> [3/6] Configuring git remotes (Dokku SSH port: ${DOKKU_SSH_PORT})"
git remote remove dokku-api 2>/dev/null || true
git remote remove dokku-web 2>/dev/null || true
if [ "$DOKKU_SSH_PORT" = "22" ]; then
  git remote add dokku-api "dokku@${SSH_HOST}:${API_APP}"
  git remote add dokku-web "dokku@${SSH_HOST}:${WEB_APP}"
else
  git remote add dokku-api "ssh://dokku@${SSH_HOST}:${DOKKU_SSH_PORT}/${API_APP}"
  git remote add dokku-web "ssh://dokku@${SSH_HOST}:${DOKKU_SSH_PORT}/${WEB_APP}"
fi

echo "==> [4/6] Pushing branches (${GIT_BRANCH} -> main on Dokku)"
git push dokku-api "${GIT_BRANCH}:main" --force
git push dokku-web "${GIT_BRANCH}:main" --force

if [ "$SKIP_LE" = "true" ]; then
  echo "==> [5/6] Skipping Let's Encrypt (SKIP_LE=true)"
else
  echo "==> [5/6] Enabling Let's Encrypt (DNS must already resolve to ${SSH_HOST})"
  # Auto-detect docker-mode dokku and wrap accordingly. setup.sh did the same.
  LE_CMD="if command -v dokku >/dev/null 2>&1; then sudo dokku letsencrypt:enable ${API_APP} && sudo dokku letsencrypt:enable ${WEB_APP}; else sudo docker exec dokku dokku letsencrypt:enable ${API_APP} && sudo docker exec dokku dokku letsencrypt:enable ${WEB_APP}; fi"
  $SSH -t "$LE_CMD" || {
    echo "WARN: LE failed. Verify DNS A records for ${API_DOMAIN} and ${WEB_DOMAIN}."
    echo "      Manually retry: ssh ${SSH_USER}@${SSH_HOST} 'sudo docker exec dokku dokku letsencrypt:enable ${API_APP}'"
  }
fi

echo "==> [6/6] Smoke tests"
SCHEME="https"
[ "$SKIP_LE" = "true" ] && SCHEME="http"
set +e
curl -fsS "${SCHEME}://${API_DOMAIN}/api/v1/health" && echo " <- api ok"
curl -fsS -o /dev/null -w "%{http_code}\n" "${SCHEME}://${WEB_DOMAIN}/" | tee /dev/null
RESET_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${SCHEME}://${API_DOMAIN}/api/v1/__test_reset")
if [ "$RESET_CODE" = "404" ]; then
  echo " <- /__test_reset blocked (404) ✓"
else
  echo "WARN: /__test_reset returned ${RESET_CODE}, expected 404. Verify nginx config."
fi
set -e

cat <<EOF

Deploy done.
  API : ${SCHEME}://${API_DOMAIN}
  WEB : ${SCHEME}://${WEB_DOMAIN}

To redeploy after code changes:
  git push dokku-api ${GIT_BRANCH}:main
  git push dokku-web ${GIT_BRANCH}:main

To rotate Turso/JWT/etc without rebuild:
  edit deploy/dokku/deploy.env  &&  bash deploy/dokku/deploy.sh
EOF
