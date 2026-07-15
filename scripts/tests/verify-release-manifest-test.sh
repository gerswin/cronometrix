#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERIFIER="${ROOT_DIR}/scripts/verify-release-manifest.sh"
FIXTURES="${ROOT_DIR}/scripts/tests/fixtures/release-manifest"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

expect_invalid() {
    local fixture="$1"
    if bash "${VERIFIER}" "${fixture}" >"${TMP_DIR}/invalid.out" 2>&1; then
        fail "expected invalid manifest: ${fixture}"
    fi
}

valid_output="$(bash "${VERIFIER}" "${FIXTURES}/valid.env")"
[[ "${valid_output}" == "manifest valid" ]] || fail "unexpected success output"

expect_invalid "${FIXTURES}/missing-digest.env"
expect_invalid "${FIXTURES}/duplicate-key.env"
expect_invalid "${FIXTURES}/extra-key.env"
expect_invalid "${FIXTURES}/shell-expansion.env"
expect_invalid "${FIXTURES}/wrong-sha.env"

cp "${FIXTURES}/writable.env" "${TMP_DIR}/writable.env"
chmod 0666 "${TMP_DIR}/writable.env"
expect_invalid "${TMP_DIR}/writable.env"

expect_invalid "${FIXTURES}/does-not-exist.env"
if bash "${VERIFIER}" >"${TMP_DIR}/arity.out" 2>&1; then
    fail "verifier accepted zero arguments"
fi
if bash "${VERIFIER}" "${FIXTURES}/valid.env" extra >"${TMP_DIR}/arity.out" 2>&1; then
    fail "verifier accepted multiple arguments"
fi

echo "PASS: release manifest validation"
