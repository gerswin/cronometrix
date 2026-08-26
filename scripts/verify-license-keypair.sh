#!/usr/bin/env bash
set -euo pipefail

if (($# != 1)); then
  echo 'usage: verify-license-keypair.sh PUBLIC_KEY_PATH' >&2
  exit 2
fi
public_key_path="$1"
[[ -f "${public_key_path}" ]] || { echo 'license public key is missing' >&2; exit 1; }
[[ -n "${CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM:-}" ]] \
  || { echo 'injected license private key is missing' >&2; exit 1; }
command -v openssl >/dev/null || { echo 'openssl is required' >&2; exit 1; }

umask 077
temporary_root="$(mktemp -d)"
cleanup() {
  rm -rf -- "${temporary_root}"
}
trap cleanup EXIT
derived_public="${temporary_root}/derived-public.pem"
normalized_public="${temporary_root}/normalized-public.pem"

if ! printf '%s\n' "${CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM}" \
  | openssl pkey -pubout -out "${derived_public}" >/dev/null 2>&1; then
  echo 'injected license private key is invalid' >&2
  exit 1
fi
if ! openssl pkey -pubin -in "${public_key_path}" -pubout \
  -out "${normalized_public}" >/dev/null 2>&1; then
  echo 'license public key is invalid' >&2
  exit 1
fi

if ! cmp -s "${derived_public}" "${normalized_public}"; then
  echo 'license keypair does not match' >&2
  exit 1
fi
echo 'license keypair verified'
