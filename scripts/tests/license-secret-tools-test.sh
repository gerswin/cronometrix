#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_root="$(mktemp -d)"
trap 'rm -rf -- "${test_root}"' EXIT

real_openssl="$(command -v openssl)"
fake_bin="${test_root}/fake-bin"
verify_bin="${test_root}/verify-bin"
mkdir -p "${fake_bin}" "${verify_bin}"

recorded_keys="${test_root}/keys"
recorded_hashes="${test_root}/hashes"
: >"${recorded_keys}"
: >"${recorded_hashes}"
export RECORDED_KEYS="${recorded_keys}"
export RECORDED_HASHES="${recorded_hashes}"

cat >"${fake_bin}/secretctl" <<'FAKE_SECRETCTL'
#!/usr/bin/env bash
set -euo pipefail
command_name="${1:-}"
shift || true
case "${command_name}" in
  list)
    cat "${RECORDED_KEYS}"
    ;;
  set)
    key="${1:?missing key}"
    value="$(cat)"
    if [[ "${key}" == cronometrix-license-test ]] \
      && [[ ! "${value}" =~ ^[A-Z0-9]{4}(-[A-Z0-9]{4}){3}$ ]]; then
      exit 9
    fi
    printf '%s\n' "${key}" >>"${RECORDED_KEYS}"
    printf '%s  %s\n' "$(printf '%s' "${value}" | shasum -a 256 | awk '{print $1}')" "${key}" \
      >>"${RECORDED_HASHES}"
    ;;
  *)
    exit 8
    ;;
esac
FAKE_SECRETCTL
chmod +x "${fake_bin}/secretctl"
cp "${fake_bin}/secretctl" "${verify_bin}/secretctl"

cat >"${fake_bin}/openssl" <<'FAKE_OPENSSL'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  x509)
    exit 0
    ;;
  genpkey)
    output=''
    while (($#)); do
      if [[ "$1" == '-out' ]]; then output="$2"; break; fi
      shift
    done
    printf '%s\n' "-----BEGIN ${PRIVATE_LABEL:-PRIVATE KEY}-----" 'test-private' \
      "-----END ${PRIVATE_LABEL:-PRIVATE KEY}-----" >"${output}"
    ;;
  pkey)
    output=''
    while (($#)); do
      if [[ "$1" == '-out' ]]; then output="$2"; break; fi
      shift
    done
    printf '%s\n' '-----BEGIN PUBLIC KEY-----' 'test-public' '-----END PUBLIC KEY-----' >"${output}"
    ;;
  *)
    exit 7
    ;;
esac
FAKE_OPENSSL
chmod +x "${fake_bin}/openssl"

ca_file="${test_root}/ca.pem"
public_key_out="${test_root}/license-public.pem"
printf '%s\n' \
  '-----BEGIN CERTIFICATE-----' \
  'ZmFrZQ==' \
  '-----END CERTIFICATE-----' >"${ca_file}"

captured_output="${test_root}/prepare-output"
PATH="${fake_bin}:${PATH}" bash "${repo_root}/scripts/prepare-license-secrets.sh" \
  --ca-file "${ca_file}" \
  --public-key-out "${public_key_out}" \
  --rotate >"${captured_output}" 2>&1

for expected_key in \
  cronometrix-aiven-license-password \
  cronometrix-aiven-ca-base64 \
  cronometrix-license-private-key-pem; do
  grep -Fxq "${expected_key}" "${recorded_keys}"
done
test -s "${public_key_out}"
if grep -q -- 'PRIVATE KEY' "${captured_output}"; then
  echo 'prepare script leaked private key material' >&2
  exit 1
fi

private_one="${test_root}/one.private.pem"
public_one="${test_root}/one.public.pem"
private_two="${test_root}/two.private.pem"
public_two="${test_root}/two.public.pem"
"${real_openssl}" genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 \
  -out "${private_one}" >/dev/null 2>&1
"${real_openssl}" pkey -in "${private_one}" -pubout -out "${public_one}" >/dev/null 2>&1
"${real_openssl}" genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 \
  -out "${private_two}" >/dev/null 2>&1
"${real_openssl}" pkey -in "${private_two}" -pubout -out "${public_two}" >/dev/null 2>&1

verify_output="${test_root}/verify-output"
PATH="${verify_bin}:${PATH}" \
CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM="$(<"${private_one}")" \
  bash "${repo_root}/scripts/verify-license-keypair.sh" "${public_one}" \
  >"${verify_output}" 2>&1
grep -Fxq 'license keypair verified' "${verify_output}"

if PATH="${verify_bin}:${PATH}" \
  CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM="$(<"${private_one}")" \
  bash "${repo_root}/scripts/verify-license-keypair.sh" "${public_two}" \
  >>"${verify_output}" 2>&1; then
  echo 'mismatched license keypair unexpectedly passed' >&2
  exit 1
fi
if grep -Eq -- 'PRIVATE KEY|^[A-Za-z0-9+/]{40,}={0,2}$' "${verify_output}"; then
  echo 'verify script leaked key material' >&2
  exit 1
fi

license_output="${test_root}/license-output"
PATH="${verify_bin}:${PATH}" bash "${repo_root}/scripts/create-license-secret.sh" \
  cronometrix-license-test >"${license_output}" 2>&1
grep -Fxq 'license secret stored: cronometrix-license-test' "${license_output}"
grep -Fxq 'cronometrix-license-test' "${recorded_keys}"

echo 'license secret tooling tests passed'
