#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALLER="${ROOT_DIR}/deploy/install.sh"
VERIFIER="${ROOT_DIR}/scripts/verify-release-manifest.sh"
FIXTURES="${ROOT_DIR}/scripts/tests/fixtures/release-manifest"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

bash -n "${INSTALLER}" || fail "installer syntax"

if grep -Eq 'install\.cronometrix\.com|curl[[:space:]]*\|[[:space:]]*bash|(:|=)latest([[:space:]"}]|$)' "${INSTALLER}"; then
    fail "installer contains public or mutable distribution path"
fi

for required in \
    'verify_release_manifest' \
    'verify_bundle' \
    'DOCKER_CONFIG' \
    '--password-stdin' \
    'read -r -s' \
    'unset CRONOMETRIX_GHCR_TOKEN' \
    'compose config --quiet' \
    'compose pull' \
    'release-manifest.env' \
    'releases/rollback' \
    'compose up -d' \
    'rollback' \
    'http://127.0.0.1:8080/api/v1/health' \
    'JWT_SECRET' \
    'DEVICE_CREDS_KEY'; do
    grep -Fq -- "${required}" "${INSTALLER}" || fail "missing installer contract: ${required}"
done

python3 - "${INSTALLER}" <<'PY'
from pathlib import Path
import sys

text = Path(sys.argv[1]).read_text()
main = text[text.index('main() {'):]
verify = main.index('verify_release_manifest "${BUNDLE_MANIFEST}"')
login = main.index('login_ghcr')
replace = main.index('install_candidate_files')
assert verify < login < replace, "manifest verification must precede login and replacement"
assert main.index('compose config --quiet') < main.index('compose pull')
assert 'LICENSE_KEY=${' not in text, "license key must not be persisted to .env"
assert 'ADMIN_PASSWORD=${' not in text, "admin password must not be persisted to .env"
assert 'cp "${COMPOSE_FILE}"' in text
assert 'cp "${MANIFEST_FILE}"' in text
assert 'compose images --format json' in text
assert 'chmod 0700 "${DOCKER_CONFIG}"' in text
assert 'chmod 0600 "${ENV_FILE}"' in text
assert 'client_max_body_size' in text
PY

run_embedded() {
    CRONOMETRIX_INSTALLER_LIBRARY=1 bash -c \
        'source "$1"; verify_release_manifest "$2"' _ "${INSTALLER}" "$1"
}

standalone_output="$(bash "${VERIFIER}" "${FIXTURES}/valid.env")"
embedded_output="$(run_embedded "${FIXTURES}/valid.env")"
[[ "${standalone_output}" == "manifest valid" ]] || fail "standalone valid output"
[[ "${embedded_output}" == "manifest valid" ]] || fail "embedded valid output"

for name in missing-digest duplicate-key extra-key shell-expansion wrong-sha; do
    if bash "${VERIFIER}" "${FIXTURES}/${name}.env" >"${TMP_DIR}/standalone" 2>&1; then
        fail "standalone accepted ${name}"
    fi
    if run_embedded "${FIXTURES}/${name}.env" >"${TMP_DIR}/embedded" 2>&1; then
        fail "embedded accepted ${name}"
    fi
done

cp "${FIXTURES}/writable.env" "${TMP_DIR}/writable.env"
chmod 0666 "${TMP_DIR}/writable.env"
if run_embedded "${TMP_DIR}/writable.env" >"${TMP_DIR}/embedded" 2>&1; then
    fail "embedded accepted writable manifest"
fi

echo "PASS: private transactional installer contract"
