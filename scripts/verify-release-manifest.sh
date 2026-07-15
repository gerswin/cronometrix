#!/usr/bin/env bash
set -euo pipefail

fail() {
    printf 'invalid manifest: %s\n' "$1" >&2
    exit 1
}

[[ "$#" -eq 1 ]] || fail "expected exactly one path"

manifest="$1"
[[ -f "${manifest}" && ! -L "${manifest}" ]] || fail "path must be a regular file"
[[ -r "${manifest}" ]] || fail "file is not readable"

if mode="$(stat -f '%Lp' "${manifest}" 2>/dev/null)"; then
    :
elif mode="$(stat -c '%a' "${manifest}" 2>/dev/null)"; then
    :
else
    fail "cannot inspect file permissions"
fi

[[ "${mode}" =~ ^[0-7]{3,4}$ ]] || fail "invalid permission mode"
mode_value=$((8#${mode}))
(( (mode_value & 8#22) == 0 )) || fail "file must not be group/world writable"

allowed_keys='|SOURCE_SHA|RELEASE_VERSION|API_IMAGE|WEB_IMAGE|GATEWAY_IMAGE|CLOUDFLARED_IMAGE|'
seen_keys='|'
key_count=0

SOURCE_SHA=''
RELEASE_VERSION=''
API_IMAGE=''
WEB_IMAGE=''
GATEWAY_IMAGE=''
CLOUDFLARED_IMAGE=''

while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ -n "${line}" ]] || fail "blank lines are not allowed"
    [[ ! "${line}" =~ [[:space:]] ]] || fail "whitespace is not allowed"
    [[ "${line}" =~ ^([A-Z_]+)=([^=]+)$ ]] || fail "malformed assignment"

    key="${BASH_REMATCH[1]}"
    value="${BASH_REMATCH[2]}"

    case "${allowed_keys}" in
        *"|${key}|"*) ;;
        *) fail "unknown key ${key}" ;;
    esac

    case "${seen_keys}" in
        *"|${key}|"*) fail "duplicate key ${key}" ;;
    esac

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

[[ "${key_count}" -eq 6 ]] || fail "manifest must contain all six keys"
[[ "${SOURCE_SHA}" =~ ^[a-f0-9]{40}$ ]] || fail "SOURCE_SHA must be 40 lowercase hex characters"
[[ "${RELEASE_VERSION}" =~ ^[A-Za-z0-9._-]+$ ]] || fail "unsafe RELEASE_VERSION"

image_pattern='^[a-z0-9./_-]+(:[A-Za-z0-9._-]+)?@sha256:[a-f0-9]{64}$'
for image in "${API_IMAGE}" "${WEB_IMAGE}" "${GATEWAY_IMAGE}" "${CLOUDFLARED_IMAGE}"; do
    [[ "${image}" =~ ${image_pattern} ]] || fail "image is not digest-pinned"
done

printf 'manifest valid\n'
