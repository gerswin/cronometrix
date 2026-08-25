#!/usr/bin/env bash
set -Eeuo pipefail
export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${SOURCE_SHA:?SOURCE_SHA required}"
: "${RELEASE_VERSION:?RELEASE_VERSION required}"
: "${API_IMAGE:?API_IMAGE required}"
: "${WEB_IMAGE:?WEB_IMAGE required}"
: "${GATEWAY_IMAGE:?GATEWAY_IMAGE required}"
: "${CLOUDFLARED_IMAGE:?CLOUDFLARED_IMAGE required}"

OUTPUT_ROOT="${CRONOMETRIX_RELEASE_OUTPUT_DIR:-${ROOT_DIR}}"
BUNDLE_DIR="${OUTPUT_ROOT}/bundle"
DIST_DIR="${OUTPUT_ROOT}/dist"

if [[ -e "${BUNDLE_DIR}" || -e "${DIST_DIR}" ]]; then
    printf 'release output must start empty: %s\n' "${OUTPUT_ROOT}" >&2
    exit 1
fi

mkdir -p "${BUNDLE_DIR}/lib" "${DIST_DIR}"
install -m 0755 "${ROOT_DIR}/deploy/install.sh" "${BUNDLE_DIR}/install.sh"
install -m 0644 "${ROOT_DIR}/deploy/docker-compose.yml" "${BUNDLE_DIR}/docker-compose.yml"
install -m 0644 "${ROOT_DIR}/deploy/nginx.conf" "${BUNDLE_DIR}/nginx.conf"
install -m 0644 "${ROOT_DIR}/deploy/lib/evidence-backup.sh" "${BUNDLE_DIR}/lib/evidence-backup.sh"

printf '%s\n' \
    "SOURCE_SHA=${SOURCE_SHA}" \
    "RELEASE_VERSION=${RELEASE_VERSION}" \
    "API_IMAGE=${API_IMAGE}" \
    "WEB_IMAGE=${WEB_IMAGE}" \
    "GATEWAY_IMAGE=${GATEWAY_IMAGE}" \
    "CLOUDFLARED_IMAGE=${CLOUDFLARED_IMAGE}" \
    > "${BUNDLE_DIR}/release-manifest.env"
chmod 0644 "${BUNDLE_DIR}/release-manifest.env"
bash "${ROOT_DIR}/scripts/verify-release-manifest.sh" "${BUNDLE_DIR}/release-manifest.env"

(
    cd "${BUNDLE_DIR}"
    sha256sum install.sh docker-compose.yml release-manifest.env nginx.conf \
        lib/evidence-backup.sh > SHA256SUMS
    sha256sum --strict -c SHA256SUMS
)

ARCHIVE="cronometrix-${RELEASE_VERSION}-${SOURCE_SHA:0:12}.tar.gz"
tar -C "${BUNDLE_DIR}" -czf "${DIST_DIR}/${ARCHIVE}" \
    install.sh docker-compose.yml release-manifest.env nginx.conf \
    lib/evidence-backup.sh SHA256SUMS

expected=$'SHA256SUMS\ndocker-compose.yml\ninstall.sh\nlib/evidence-backup.sh\nnginx.conf\nrelease-manifest.env'
actual="$(tar -tzf "${DIST_DIR}/${ARCHIVE}" | sort)"
[[ "${actual}" == "${expected}" ]] || {
    printf 'release archive has an unexpected member set\n' >&2
    exit 1
}

(
    cd "${DIST_DIR}"
    sha256sum "${ARCHIVE}" > "${ARCHIVE}.sha256"
    sha256sum --strict -c "${ARCHIVE}.sha256"
)
