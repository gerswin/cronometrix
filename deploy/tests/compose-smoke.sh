#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="${ROOT_DIR}/deploy/docker-compose.local.yml"
TMP_DIR="$(mktemp -d /tmp/cronometrix-container-smoke-XXXXXXXX)"
PROJECT="cronometrix-smoke-$$"
PORT="${GATEWAY_HOST_PORT:-18080}"
BASE_URL="http://127.0.0.1:${PORT}"
ARTIFACT_DIR="${ROOT_DIR}/deploy/test-artifacts"
ARTIFACT="${ARTIFACT_DIR}/container-smoke.log"
ENV_FILE="${TMP_DIR}/smoke.env"
export CRONOMETRIX_DATA_DIR="${TMP_DIR}/data"
export GATEWAY_HOST_PORT="${PORT}"
export CRONOMETRIX_LOCAL_ENV_FILE="${ENV_FILE}"

compose() {
    docker compose --project-name "${PROJECT}" -f "${COMPOSE_FILE}" --env-file "${ENV_FILE}" "$@"
}

cleanup() {
    local status=$?
    set +e
    mkdir -p "${ARTIFACT_DIR}"
    compose logs --no-color gateway api web > "${ARTIFACT}" 2>&1
    compose down --volumes --remove-orphans >/dev/null 2>&1
    rm -rf "${TMP_DIR}"
    printf 'container smoke cleanup complete\n' >&2
    exit "${status}"
}
trap cleanup EXIT

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

wait_url() {
    local url="$1" attempt
    for attempt in $(seq 1 90); do
        curl -fsS "${url}" >/dev/null 2>&1 && return 0
        [[ "${attempt}" -lt 90 ]] || break
        sleep 2
    done
    return 1
}

assert_marker_absent() {
    local snapshot="${TMP_DIR}/compose-logs.txt"
    compose logs --no-color gateway api web > "${snapshot}" 2>&1
    ! grep -Fq -- "${SSE_LOG_MARKER_TOKEN}" "${snapshot}" || fail "successful SSE token leaked to container logs"
    ! grep -Fq -- "${SSE_INVALID_MARKER}" "${snapshot}" || fail "invalid SSE marker leaked to container logs"
    cp "${snapshot}" "${ARTIFACT}"
    ! grep -Fq -- "${SSE_LOG_MARKER_TOKEN}" "${ARTIFACT}" || fail "successful SSE token leaked to redacted artifact"
    ! grep -Fq -- "${SSE_INVALID_MARKER}" "${ARTIFACT}" || fail "invalid SSE marker leaked to redacted artifact"
}

mkdir -p "${CRONOMETRIX_DATA_DIR}" "${ARTIFACT_DIR}"
chmod 0700 "${CRONOMETRIX_DATA_DIR}"
JWT_SECRET="$(openssl rand -hex 32)"
REFRESH_TOKEN_SECRET="$(openssl rand -hex 32)"
DEVICE_CREDS_KEY="$(openssl rand -base64 32 | tr -d '\n')"
ADMIN_PASSWORD="smoke-$(openssl rand -hex 12)"
cat > "${ENV_FILE}" <<EOF
JWT_SECRET=${JWT_SECRET}
REFRESH_TOKEN_SECRET=${REFRESH_TOKEN_SECRET}
DEVICE_CREDS_KEY=${DEVICE_CREDS_KEY}
TURSO_DATABASE_URL=
TURSO_AUTH_TOKEN=
DO_FUNCTIONS_ACTIVATE_URL=
DO_FUNCTIONS_RENEW_URL=
LICENSE_PUBKEY_PATH=/opt/cronometrix/data/license_pubkey.pem
EOF
chmod 0600 "${ENV_FILE}"

compose build api web gateway
compose up -d api web gateway
wait_url "${BASE_URL}/gateway-health" || fail "gateway did not become ready"

API_BINDINGS="$(docker inspect -f '{{json .HostConfig.PortBindings}}' "$(compose ps -q api)")"
WEB_BINDINGS="$(docker inspect -f '{{json .HostConfig.PortBindings}}' "$(compose ps -q web)")"
[[ "${API_BINDINGS}" == "null" || "${API_BINDINGS}" == "{}" ]] || fail "API port is published"
[[ "${WEB_BINDINGS}" == "null" || "${WEB_BINDINGS}" == "{}" ]] || fail "web port is published"
[[ "$(curl -fsS "${BASE_URL}/gateway-health")" == "ok" ]] || fail "gateway health body"
curl -fsS "${BASE_URL}/api/v1/health" >/dev/null
curl -fsS "${BASE_URL}/login" > "${TMP_DIR}/login.html"
grep -Fq 'Cronometrix' "${TMP_DIR}/login.html" || fail "login page content"
! grep -Fq 'http://api:3001' "${TMP_DIR}/login.html" || fail "internal API leaked into HTML"
grep -oE 'src="[^"]+\.js' "${TMP_DIR}/login.html" | cut -d'"' -f2 | while IFS= read -r asset; do
    curl -fsS "${BASE_URL}${asset}"
done > "${TMP_DIR}/browser.js"
! grep -Fq 'http://api:3001' "${TMP_DIR}/browser.js" || fail "internal API leaked into browser JS"

# License bypass enables the first E2E capability only. Without the separate
# CRONOMETRIX_TEST_RESET_ENABLED capability the destructive route stays absent.
[[ "$(curl -sS -o /dev/null -w '%{http_code}' -X POST "${BASE_URL}/api/v1/__test_reset")" == "404" ]] || fail "test reset route must remain disabled"

python3 - "${ADMIN_PASSWORD}" <<'PY' | curl -fsS -H 'Content-Type: application/json' --data-binary @- "${BASE_URL}/api/v1/setup/init" >/dev/null
import json, sys
print(json.dumps({"full_name": "Smoke Admin", "username": "smoke-admin", "password": sys.argv[1]}))
PY
LOGIN_RESPONSE="$(python3 - "${ADMIN_PASSWORD}" <<'PY' | curl -fsS -H 'Content-Type: application/json' --data-binary @- "${BASE_URL}/api/v1/auth/login"
import json, sys
print(json.dumps({"username": "smoke-admin", "password": sys.argv[1]}))
PY
)"
SSE_LOG_MARKER_TOKEN="$(printf '%s' "${LOGIN_RESPONSE}" | python3 -c 'import json,sys,urllib.parse; print(urllib.parse.quote(json.load(sys.stdin)["access_token"], safe=""))')"
SSE_INVALID_MARKER="invalid-$(openssl rand -hex 20)"
unset LOGIN_RESPONSE ADMIN_PASSWORD

set +e
timeout 20 curl -NsS "${BASE_URL}/api/v1/events/stream?token=${SSE_LOG_MARKER_TOKEN}" > "${TMP_DIR}/sse.out" 2>/dev/null
SSE_EXIT=$?
set -e
[[ "${SSE_EXIT}" -eq 124 || "${SSE_EXIT}" -eq 0 ]] || fail "successful SSE stream"
[[ -s "${TMP_DIR}/sse.out" ]] || fail "SSE heartbeat was not observed"
assert_marker_absent

[[ "$(curl -sS -o /dev/null -w '%{http_code}' "${BASE_URL}/api/v1/events/stream?token=${SSE_INVALID_MARKER}")" == "401" ]] || fail "invalid SSE status"
assert_marker_absent

compose stop api >/dev/null
curl -sS --max-time 5 -o /dev/null "${BASE_URL}/api/v1/events/stream?token=${SSE_LOG_MARKER_TOKEN}" || true
assert_marker_absent
compose start api >/dev/null
wait_url "${BASE_URL}/api/v1/health" || fail "API did not recover"

truncate -s 2M "${TMP_DIR}/photo.jpg"
PHOTO_STATUS="$(curl -sS -o /dev/null -w '%{http_code}' -F "photo=@${TMP_DIR}/photo.jpg" "${BASE_URL}/api/v1/setup/status")"
[[ "${PHOTO_STATUS}" != "413" ]] || fail "multipart photo blocked by gateway"
truncate -s 10M "${TMP_DIR}/evidence.bin"
EVIDENCE_STATUS="$(curl -sS -o /dev/null -w '%{http_code}' -H 'Content-Type: application/octet-stream' --data-binary "@${TMP_DIR}/evidence.bin" "${BASE_URL}/api/v1/setup/status")"
[[ "${EVIDENCE_STATUS}" != "413" ]] || fail "10 MiB evidence blocked by gateway"

compose restart gateway >/dev/null
wait_url "${BASE_URL}/login" || fail "frontend did not recover after gateway restart"
wait_url "${BASE_URL}/api/v1/health" || fail "API did not recover after gateway restart"
curl -fsS "${BASE_URL}/api/v1/setup/status" | python3 -c 'import json,sys; assert json.load(sys.stdin)["initialized"] is True'

assert_marker_absent
for secret in "${JWT_SECRET}" "${REFRESH_TOKEN_SECRET}" "${DEVICE_CREDS_KEY}"; do
    ! grep -Fq -- "${secret}" "${ARTIFACT}" || fail "generated secret leaked to logs"
done
! grep -Fqi 'database is locked' "${ARTIFACT}" || fail "database lock error in logs"
unset SSE_LOG_MARKER_TOKEN SSE_INVALID_MARKER JWT_SECRET REFRESH_TOKEN_SECRET DEVICE_CREDS_KEY

echo "PASS: same-origin container topology"
