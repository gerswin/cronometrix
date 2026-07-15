#!/usr/bin/env bash
# Cronometrix private release installer.
# Transfer and extract the authenticated Actions artifact, then run:
#   sudo bash install.sh

set -Eeuo pipefail

log() { printf '[install] %s\n' "$*" >&2; }
die() {
    printf '[install] ERROR: %s\n' "$*" >&2
    if [[ "${TRANSACTION_ACTIVE:-0}" -eq 1 ]]; then
        rollback 1
    fi
    exit 1
}

verify_release_manifest() {
    [[ "$#" -eq 1 ]] || { printf 'invalid manifest: expected exactly one path\n' >&2; return 1; }
    local manifest="$1" mode mode_value line key value image
    local allowed_keys='|SOURCE_SHA|RELEASE_VERSION|API_IMAGE|WEB_IMAGE|GATEWAY_IMAGE|CLOUDFLARED_IMAGE|'
    local seen_keys='|' key_count=0
    local SOURCE_SHA='' RELEASE_VERSION='' API_IMAGE='' WEB_IMAGE='' GATEWAY_IMAGE='' CLOUDFLARED_IMAGE=''
    local image_pattern='^[a-z0-9./_-]+(:[A-Za-z0-9._-]+)?@sha256:[a-f0-9]{64}$'

    [[ -f "${manifest}" && ! -L "${manifest}" && -r "${manifest}" ]] || {
        printf 'invalid manifest: path must be a readable regular file\n' >&2
        return 1
    }
    if mode="$(stat -f '%Lp' "${manifest}" 2>/dev/null)"; then
        :
    elif mode="$(stat -c '%a' "${manifest}" 2>/dev/null)"; then
        :
    else
        printf 'invalid manifest: cannot inspect file permissions\n' >&2
        return 1
    fi
    [[ "${mode}" =~ ^[0-7]{3,4}$ ]] || return 1
    mode_value=$((8#${mode}))
    (( (mode_value & 8#22) == 0 )) || {
        printf 'invalid manifest: file must not be group/world writable\n' >&2
        return 1
    }

    while IFS= read -r line || [[ -n "${line}" ]]; do
        [[ -n "${line}" && ! "${line}" =~ [[:space:]] ]] || return 1
        [[ "${line}" =~ ^([A-Z_]+)=([^=]+)$ ]] || return 1
        key="${BASH_REMATCH[1]}"
        value="${BASH_REMATCH[2]}"
        case "${allowed_keys}" in *"|${key}|"*) ;; *) return 1 ;; esac
        case "${seen_keys}" in *"|${key}|"*) return 1 ;; esac
        seen_keys="${seen_keys}${key}|"
        key_count=$((key_count + 1))
        case "${key}" in
            SOURCE_SHA) SOURCE_SHA="${value}" ;;
            RELEASE_VERSION) RELEASE_VERSION="${value}" ;;
            API_IMAGE) API_IMAGE="${value}" ;;
            WEB_IMAGE) WEB_IMAGE="${value}" ;;
            GATEWAY_IMAGE) GATEWAY_IMAGE="${value}" ;;
            CLOUDFLARED_IMAGE) CLOUDFLARED_IMAGE="${value}" ;;
        esac
    done < "${manifest}"

    [[ "${key_count}" -eq 6 ]] || return 1
    [[ "${SOURCE_SHA}" =~ ^[a-f0-9]{40}$ ]] || return 1
    [[ "${RELEASE_VERSION}" =~ ^[A-Za-z0-9._-]+$ ]] || return 1
    for image in "${API_IMAGE}" "${WEB_IMAGE}" "${GATEWAY_IMAGE}" "${CLOUDFLARED_IMAGE}"; do
        [[ "${image}" =~ ${image_pattern} ]] || return 1
    done
    printf 'manifest valid\n'
}

if [[ "${CRONOMETRIX_INSTALLER_LIBRARY:-}" == "1" ]]; then
    if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
        return 0
    fi
    exit 0
fi

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUNDLE_MANIFEST="${BUNDLE_DIR}/release-manifest.env"
INSTALL_DIR="${CRONOMETRIX_INSTALL_DIR:-/opt/cronometrix}"
DATA_DIR="${INSTALL_DIR}/data"
ENV_FILE="${INSTALL_DIR}/.env"
COMPOSE_FILE="${INSTALL_DIR}/docker-compose.yml"
MANIFEST_FILE="${INSTALL_DIR}/release-manifest.env"
NGINX_FILE="${INSTALL_DIR}/nginx.conf"
ROLLBACK_ROOT="${INSTALL_DIR}/releases/rollback"
DOCKER_CONFIG="${INSTALL_DIR}/.docker"
export DOCKER_CONFIG
BACKUP_DIR=''
HAD_PREVIOUS=0
TRANSACTION_ACTIVE=0

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

manifest_value() {
    local key="$1"
    sed -n "s/^${key}=//p" "${BUNDLE_MANIFEST}"
}

verify_bundle() {
    local expected actual member
    expected="$(printf '%s\n' SHA256SUMS docker-compose.yml install.sh nginx.conf release-manifest.env | sort)"
    actual="$(find "${BUNDLE_DIR}" -mindepth 1 -maxdepth 1 -print | sed 's#^.*/##' | sort)"
    [[ "${actual}" == "${expected}" ]] || die "bundle must contain exactly the five approved files"
    for member in install.sh docker-compose.yml release-manifest.env nginx.conf SHA256SUMS; do
        [[ -f "${BUNDLE_DIR}/${member}" && ! -L "${BUNDLE_DIR}/${member}" ]] || die "invalid bundle member: ${member}"
    done
    [[ "$(awk '{print $2}' "${BUNDLE_DIR}/SHA256SUMS" | sed 's/^\*//' | sort)" == \
       "$(printf '%s\n' docker-compose.yml install.sh nginx.conf release-manifest.env | sort)" ]] || \
        die "SHA256SUMS must cover exactly the other four bundle files"
    (cd "${BUNDLE_DIR}" && sha256sum --strict -c SHA256SUMS >/dev/null) || die "bundle checksum verification failed"
}

version_at_least() {
    [[ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n1)" == "$2" ]]
}

preflight() {
    [[ "$(id -u)" -eq 0 ]] || die "must run as root (sudo)"
    [[ "$(uname -s)" == "Linux" ]] || die "supported platform is Linux amd64"
    case "$(uname -m)" in x86_64|amd64) ;; *) die "supported architecture is amd64" ;; esac
    for cmd in docker openssl curl python3 sha256sum awk sed grep find sort df install jq truncate; do
        require_cmd "${cmd}"
    done
    docker compose version >/dev/null 2>&1 || die "Docker Compose v2 is required"
    docker version --format '{{.Server.Version}}' >/dev/null 2>&1 || die "Docker daemon is unavailable"
    local docker_version compose_version parent available_kb
    docker_version="$(docker version --format '{{.Server.Version}}')"
    compose_version="$(docker compose version --short)"
    version_at_least "${docker_version}" 24.0.0 || die "Docker 24.0.0 or newer is required"
    version_at_least "${compose_version}" 2.24.0 || die "Docker Compose 2.24.0 or newer is required"
    parent="$(dirname "${INSTALL_DIR}")"
    [[ -d "${parent}" && -w "${parent}" ]] || die "install parent is not writable: ${parent}"
    available_kb="$(df -Pk "${parent}" | awk 'NR==2 {print $4}')"
    [[ "${available_kb}" =~ ^[0-9]+$ && "${available_kb}" -ge 2097152 ]] || die "at least 2 GiB free space is required"
}

read_inputs() {
    log "Cronometrix private installer"
    if [[ -t 0 ]]; then
        read -r -p "GHCR username: " CRONOMETRIX_GHCR_USERNAME
        read -r -s -p "GHCR read token: " CRONOMETRIX_GHCR_TOKEN; printf '\n' >&2
        read -r -s -p "License key: " CRONOMETRIX_LICENSE_KEY; printf '\n' >&2
        read -r -p "Client slug: " CRONOMETRIX_CLIENT_SLUG
        read -r -s -p "Admin password: " CRONOMETRIX_ADMIN_PASSWORD; printf '\n' >&2
        read -r -s -p "Cloudflare tunnel token: " CRONOMETRIX_CF_TUNNEL_TOKEN; printf '\n' >&2
        read -r -p "DigitalOcean activation URL: " CRONOMETRIX_DO_ACTIVATE_URL
        read -r -p "DigitalOcean renewal URL: " CRONOMETRIX_DO_RENEW_URL
    else
        : "${CRONOMETRIX_GHCR_USERNAME:?CRONOMETRIX_GHCR_USERNAME required in non-interactive mode}"
        : "${CRONOMETRIX_GHCR_TOKEN:?CRONOMETRIX_GHCR_TOKEN required in non-interactive mode}"
        : "${CRONOMETRIX_LICENSE_KEY:?CRONOMETRIX_LICENSE_KEY required in non-interactive mode}"
        : "${CRONOMETRIX_CLIENT_SLUG:?CRONOMETRIX_CLIENT_SLUG required in non-interactive mode}"
        : "${CRONOMETRIX_ADMIN_PASSWORD:?CRONOMETRIX_ADMIN_PASSWORD required in non-interactive mode}"
        : "${CRONOMETRIX_CF_TUNNEL_TOKEN:?CRONOMETRIX_CF_TUNNEL_TOKEN required in non-interactive mode}"
        : "${CRONOMETRIX_DO_ACTIVATE_URL:?CRONOMETRIX_DO_ACTIVATE_URL required in non-interactive mode}"
        : "${CRONOMETRIX_DO_RENEW_URL:?CRONOMETRIX_DO_RENEW_URL required in non-interactive mode}"
    fi

    [[ "${CRONOMETRIX_GHCR_USERNAME}" =~ ^[A-Za-z0-9_.-]+$ ]] || die "invalid GHCR username"
    [[ "${CRONOMETRIX_LICENSE_KEY}" =~ ^[A-Za-z0-9]{4}-[A-Za-z0-9]{4}-[A-Za-z0-9]{4}-[A-Za-z0-9]{4}$ ]] || die "invalid license key format"
    [[ "${CRONOMETRIX_CLIENT_SLUG}" =~ ^[a-z0-9][a-z0-9-]{1,62}[a-z0-9]$ ]] || die "invalid client slug"
    [[ "${#CRONOMETRIX_ADMIN_PASSWORD}" -ge 8 ]] || die "admin password must be at least 8 characters"
    [[ "${CRONOMETRIX_DO_ACTIVATE_URL}" =~ ^https://[^[:space:]]+$ ]] || die "activation URL must use HTTPS"
    [[ "${CRONOMETRIX_DO_RENEW_URL}" =~ ^https://[^[:space:]]+$ ]] || die "renewal URL must use HTTPS"
    for value in "${CRONOMETRIX_GHCR_TOKEN}" "${CRONOMETRIX_CF_TUNNEL_TOKEN}" "${CRONOMETRIX_ADMIN_PASSWORD}"; do
        [[ "${value}" != *$'\n'* && "${value}" != *$'\r'* ]] || die "secret inputs cannot contain newlines"
    done
}

read_existing_secret() {
    local key="$1"
    awk -F= -v key="${key}" '$1 == key {sub(/^[^=]*=/, ""); print; exit}' "${ENV_FILE}"
}

prepare_directories_and_secrets() {
    umask 077
    mkdir -p "${INSTALL_DIR}" "${DATA_DIR}" "${ROLLBACK_ROOT}" "${DOCKER_CONFIG}"
    chmod 0700 "${INSTALL_DIR}" "${DATA_DIR}" "${INSTALL_DIR}/releases" "${ROLLBACK_ROOT}" "${DOCKER_CONFIG}"
    if [[ -f "${ENV_FILE}" ]]; then
        JWT_SECRET="$(read_existing_secret JWT_SECRET)"
        DEVICE_CREDS_KEY="$(read_existing_secret DEVICE_CREDS_KEY)"
        [[ -n "${JWT_SECRET}" && -n "${DEVICE_CREDS_KEY}" ]] || die "existing .env is missing JWT_SECRET or DEVICE_CREDS_KEY"
        log "preserving JWT_SECRET, DEVICE_CREDS_KEY, data, and Docker credentials"
    else
        JWT_SECRET="$(openssl rand -hex 32)"
        DEVICE_CREDS_KEY="$(openssl rand -base64 32 | tr -d '\n')"
    fi
}

login_ghcr() {
    printf '%s' "${CRONOMETRIX_GHCR_TOKEN}" | \
        docker --config "${DOCKER_CONFIG}" login ghcr.io \
            --username "${CRONOMETRIX_GHCR_USERNAME}" --password-stdin >/dev/null
    unset CRONOMETRIX_GHCR_TOKEN
    chmod 0700 "${DOCKER_CONFIG}"
    [[ ! -f "${DOCKER_CONFIG}/config.json" ]] || chmod 0600 "${DOCKER_CONFIG}/config.json"
}

compose() {
    docker compose --project-directory "${INSTALL_DIR}" \
        --env-file "${ENV_FILE}" --env-file "${MANIFEST_FILE}" \
        -f "${COMPOSE_FILE}" "$@"
}

backup_existing_release() {
    local stamp db_path
    stamp="$(date -u +%Y%m%dT%H%M%SZ)"
    BACKUP_DIR="${ROLLBACK_ROOT}/${stamp}"
    mkdir -p "${BACKUP_DIR}"
    chmod 0700 "${BACKUP_DIR}"
    if [[ -f "${COMPOSE_FILE}" && -f "${MANIFEST_FILE}" ]]; then
        HAD_PREVIOUS=1
        cp "${COMPOSE_FILE}" "${BACKUP_DIR}/docker-compose.yml"
        cp "${MANIFEST_FILE}" "${BACKUP_DIR}/release-manifest.env"
        [[ ! -f "${INSTALL_DIR}/install.sh" ]] || cp "${INSTALL_DIR}/install.sh" "${BACKUP_DIR}/install.sh"
        [[ ! -f "${NGINX_FILE}" ]] || cp "${NGINX_FILE}" "${BACKUP_DIR}/nginx.conf"
        [[ ! -f "${ENV_FILE}" ]] || cp "${ENV_FILE}" "${BACKUP_DIR}/.env"
        compose images --format json > "${BACKUP_DIR}/container-images.json" 2>/dev/null || true
    fi
    db_path="${DATA_DIR}/cronometrix.db"
    if [[ -f "${db_path}" ]]; then
        python3 - "${db_path}" "${BACKUP_DIR}/cronometrix.db" <<'PY'
import sqlite3
import sys
source = sqlite3.connect(sys.argv[1])
target = sqlite3.connect(sys.argv[2])
with target:
    source.backup(target)
target.close()
source.close()
PY
        chmod 0600 "${BACKUP_DIR}/cronometrix.db"
    fi
}

write_runtime_env() {
    local candidate="${INSTALL_DIR}/.env.candidate"
    umask 077
    {
        printf 'JWT_SECRET=%s\n' "${JWT_SECRET}"
        printf 'DEVICE_CREDS_KEY=%s\n' "${DEVICE_CREDS_KEY}"
        printf 'CLOUDFLARE_TUNNEL_TOKEN=%s\n' "${CRONOMETRIX_CF_TUNNEL_TOKEN}"
        printf 'CLIENT_SLUG=%s\n' "${CRONOMETRIX_CLIENT_SLUG}"
        printf 'LICENSE_JWT_PATH=/opt/cronometrix/data/license.jwt\n'
        printf 'DO_FUNCTIONS_ACTIVATE_URL=%s\n' "${CRONOMETRIX_DO_ACTIVATE_URL}"
        printf 'DO_FUNCTIONS_RENEW_URL=%s\n' "${CRONOMETRIX_DO_RENEW_URL}"
        printf 'TZ=%s\n' "${CRONOMETRIX_TZ:-America/Caracas}"
        printf 'CRONOMETRIX_DB_PATH=/opt/cronometrix/data/cronometrix.db\n'
        printf 'SERVER_HOST=0.0.0.0\nSERVER_PORT=3001\n'
    } > "${candidate}"
    chmod 0600 "${candidate}"
    mv -f "${candidate}" "${ENV_FILE}"
    unset CRONOMETRIX_CF_TUNNEL_TOKEN
}

install_candidate_files() {
    install -m 0755 "${BUNDLE_DIR}/install.sh" "${INSTALL_DIR}/install.sh.candidate"
    install -m 0644 "${BUNDLE_DIR}/docker-compose.yml" "${COMPOSE_FILE}.candidate"
    install -m 0644 "${BUNDLE_MANIFEST}" "${MANIFEST_FILE}.candidate"
    install -m 0644 "${BUNDLE_DIR}/nginx.conf" "${NGINX_FILE}.candidate"
    mv -f "${INSTALL_DIR}/install.sh.candidate" "${INSTALL_DIR}/install.sh"
    mv -f "${COMPOSE_FILE}.candidate" "${COMPOSE_FILE}"
    mv -f "${MANIFEST_FILE}.candidate" "${MANIFEST_FILE}"
    mv -f "${NGINX_FILE}.candidate" "${NGINX_FILE}"
}

wait_api_internal() {
    local attempt
    for attempt in $(seq 1 30); do
        if compose exec -T api curl -fsS http://127.0.0.1:3001/api/v1/health >/dev/null 2>&1; then
            return 0
        fi
        [[ "${attempt}" -lt 30 ]] || break
        sleep 2
    done
    return 1
}

wait_gateway() {
    local attempt
    for attempt in $(seq 1 30); do
        if curl -fsS http://127.0.0.1:8080/api/v1/health >/dev/null 2>&1; then
            return 0
        fi
        [[ "${attempt}" -lt 30 ]] || break
        sleep 2
    done
    return 1
}

rollback() {
    local status="${1:-1}"
    trap - ERR
    set +e
    TRANSACTION_ACTIVE=0
    log "candidate failed; restoring previous release"
    if [[ "${HAD_PREVIOUS}" -eq 1 ]]; then
        compose down >/dev/null 2>&1 || true
        cp "${BACKUP_DIR}/docker-compose.yml" "${COMPOSE_FILE}.rollback"
        cp "${BACKUP_DIR}/release-manifest.env" "${MANIFEST_FILE}.rollback"
        mv -f "${COMPOSE_FILE}.rollback" "${COMPOSE_FILE}"
        mv -f "${MANIFEST_FILE}.rollback" "${MANIFEST_FILE}"
        if [[ -f "${BACKUP_DIR}/install.sh" ]]; then
            cp "${BACKUP_DIR}/install.sh" "${INSTALL_DIR}/install.sh.rollback"
            chmod 0755 "${INSTALL_DIR}/install.sh.rollback"
            mv -f "${INSTALL_DIR}/install.sh.rollback" "${INSTALL_DIR}/install.sh"
        fi
        if [[ -f "${BACKUP_DIR}/nginx.conf" ]]; then
            cp "${BACKUP_DIR}/nginx.conf" "${NGINX_FILE}.rollback"
            mv -f "${NGINX_FILE}.rollback" "${NGINX_FILE}"
        fi
        if [[ -f "${BACKUP_DIR}/.env" ]]; then
            cp "${BACKUP_DIR}/.env" "${ENV_FILE}.rollback"
            chmod 0600 "${ENV_FILE}.rollback"
            mv -f "${ENV_FILE}.rollback" "${ENV_FILE}"
        fi
        if [[ -f "${BACKUP_DIR}/cronometrix.db" ]]; then
            cp "${BACKUP_DIR}/cronometrix.db" "${DATA_DIR}/cronometrix.db.rollback"
            chmod 0600 "${DATA_DIR}/cronometrix.db.rollback"
            mv -f "${DATA_DIR}/cronometrix.db.rollback" "${DATA_DIR}/cronometrix.db"
            rm -f "${DATA_DIR}/cronometrix.db-wal" "${DATA_DIR}/cronometrix.db-shm"
        fi
        compose up -d
        wait_gateway || log "WARNING: previous release did not recover health; inspect ${BACKUP_DIR}"
    else
        compose down >/dev/null 2>&1 || true
    fi
    exit "${status}"
}

activate_license() {
    local key_hash_file="${DATA_DIR}/.license-key.sha256" supplied_hash response status
    supplied_hash="$(printf '%s' "${CRONOMETRIX_LICENSE_KEY}" | sha256sum | awk '{print $1}')"
    if [[ -f "${DATA_DIR}/license.jwt" ]]; then
        [[ -f "${key_hash_file}" ]] || die "existing license cannot be matched to supplied key; contact support"
        [[ "$(cat "${key_hash_file}")" == "${supplied_hash}" ]] || die "supplied license does not match this installation"
        curl -fsS http://127.0.0.1:8080/api/v1/setup/status | \
            python3 -c 'import json,sys; assert json.load(sys.stdin).get("licensed") is True'
        unset CRONOMETRIX_LICENSE_KEY
        log "matching license already active"
        return 0
    fi

    response="${INSTALL_DIR}/.activate-response"
    status="$(
        printf '%s' "${CRONOMETRIX_LICENSE_KEY}" | \
            python3 -c 'import json,sys; print(json.dumps({"license_key": sys.stdin.read()}))' | \
            curl -sS -o "${response}" -w '%{http_code}' \
                -H 'Content-Type: application/json' --data-binary @- \
                http://127.0.0.1:8080/api/v1/setup/activate
    )"
    [[ "${status}" == "200" ]] || die "license activation failed with HTTP ${status}"
    python3 - "${response}" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    assert json.load(stream).get("activated") is True
PY
    printf '%s\n' "${supplied_hash}" > "${key_hash_file}"
    chmod 0600 "${key_hash_file}"
    rm -f "${response}"
    unset CRONOMETRIX_LICENSE_KEY
}

initialize_admin() {
    local response="${INSTALL_DIR}/.setup-response" status
    status="$(
        printf '%s' "${CRONOMETRIX_ADMIN_PASSWORD}" | \
            python3 -c 'import json,sys; print(json.dumps({"full_name":"Administrator","username":"admin","password":sys.stdin.read()}))' | \
            curl -sS -o "${response}" -w '%{http_code}' \
                -H 'Content-Type: application/json' --data-binary @- \
                http://127.0.0.1:8080/api/v1/setup/init
    )"
    case "${status}" in
        201) log "initial administrator created" ;;
        409) grep -Fq 'SETUP_ALREADY_COMPLETE' "${response}" || die "unexpected setup conflict" ;;
        *) die "administrator setup failed with HTTP ${status}" ;;
    esac
    rm -f "${response}"
    unset CRONOMETRIX_ADMIN_PASSWORD
}

verify_candidate_health() {
    curl -fsS http://127.0.0.1:8080/gateway-health >/dev/null
    curl -fsS http://127.0.0.1:8080/api/v1/health >/dev/null
    curl -fsS http://127.0.0.1:8080/api/v1/setup/status | python3 -m json.tool >/dev/null
    local upload_status upload_probe="${INSTALL_DIR}/.upload-limit-probe"
    truncate -s 13M "${upload_probe}"
    upload_status="$(curl -sS -o /dev/null -w '%{http_code}' \
        -X POST -H 'Content-Type: application/octet-stream' --data-binary "@${upload_probe}" \
        http://127.0.0.1:8080/api/v1/setup/status)"
    rm -f "${upload_probe}"
    [[ "${upload_status}" == "413" ]] || die "gateway client_max_body_size contract failed"
    [[ "$(compose ps --format json api | jq -r 'if type == "array" then .[0].Health else .Health end')" == "healthy" ]] || die "api container is not healthy"
    [[ "$(compose ps --format json gateway | jq -r 'if type == "array" then .[0].Health else .Health end')" == "healthy" ]] || die "gateway container is not healthy"
}

prune_rollbacks() {
    local old
    while IFS= read -r old; do
        [[ -n "${old}" ]] && rm -rf -- "${old}"
    done < <(find "${ROLLBACK_ROOT}" -mindepth 1 -maxdepth 1 -type d -print | sort -r | tail -n +3)
}

main() {
    [[ "$#" -eq 0 ]] || die "positional arguments are not accepted"
    verify_bundle
    verify_release_manifest "${BUNDLE_MANIFEST}" >/dev/null
    preflight
    read_inputs
    prepare_directories_and_secrets
    login_ghcr
    backup_existing_release
    TRANSACTION_ACTIVE=1
    trap 'status=$?; [[ "${TRANSACTION_ACTIVE}" -eq 0 ]] || rollback "${status}"' ERR
    write_runtime_env
    install_candidate_files
    compose config --quiet
    compose pull
    compose stop cloudflared >/dev/null 2>&1 || true
    compose up -d api
    wait_api_internal || die "api failed internal health check"
    compose up -d web gateway
    wait_gateway || die "gateway/API failed health check"
    activate_license
    initialize_admin
    verify_candidate_health
    compose up -d cloudflared
    compose ps --status running --services | grep -Fxq cloudflared || die "cloudflared is not running"

    TRANSACTION_ACTIVE=0
    trap - ERR
    prune_rollbacks
    chmod 0600 "${ENV_FILE}"
    chmod 0700 "${DATA_DIR}" "${DOCKER_CONFIG}"
    log "installation complete: http://127.0.0.1:8080"
    log "public hostname: https://${CRONOMETRIX_CLIENT_SLUG}.cronometrix.com"
}

main "$@"
