#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_root="$(mktemp -d)"
trap 'rm -rf -- "${test_root}"' EXIT
fake_bin="${test_root}/bin"
mkdir -p "${fake_bin}"
record="${test_root}/calls"
: >"${record}"
export FAKE_CALLS="${record}"

cat >"${fake_bin}/git" <<'FAKE_GIT'
#!/usr/bin/env bash
set -euo pipefail
printf 'git %s\n' "$*" >>"${FAKE_CALLS}"
case "${1:-}" in
  fetch) exit 0 ;;
  status) printf '%s' "${FAKE_GIT_DIRTY:-}" ;;
  rev-parse)
    if [[ "${2:-}" == 'origin/main' ]]; then
      printf '%s\n' "${FAKE_ORIGIN_SHA:-1111111111111111111111111111111111111111}"
    else
      printf '%s\n' "${FAKE_HEAD_SHA:-1111111111111111111111111111111111111111}"
    fi
    ;;
  *) exit 3 ;;
esac
FAKE_GIT

cat >"${fake_bin}/npm" <<'FAKE_NPM'
#!/usr/bin/env bash
set -euo pipefail
printf 'npm %s\n' "$*" >>"${FAKE_CALLS}"
printf '%s\n' 'postgres://runtime:runtime-secret@db.example/cronometrix_licenses?sslmode=require'
FAKE_NPM

cat >"${fake_bin}/doctl" <<'FAKE_DOCTL'
#!/usr/bin/env bash
set -euo pipefail
printf 'doctl %s\n' "$*" >>"${FAKE_CALLS}"
if [[ "$*" == 'serverless namespaces list --format Label --no-header' ]]; then
  printf '%b\n' "${FAKE_NAMESPACE_LABELS:-cronometrix-old\nother}"
elif [[ "$*" == 'serverless functions get licenses/activate --url' ]]; then
  printf '%s\n' "${FAKE_ACTIVATE_URL:-https://functions.example/activate}"
elif [[ "$*" == 'serverless functions get licenses/renew --url' ]]; then
  printf '%s\n' "${FAKE_RENEW_URL:-https://functions.example/renew}"
fi
FAKE_DOCTL

cat >"${fake_bin}/curl" <<'FAKE_CURL'
#!/usr/bin/env bash
set -euo pipefail
printf 'curl invoked\n' >>"${FAKE_CALLS}"
output=''
data=''
while (($#)); do
  case "$1" in
    --output) output="$2"; shift 2 ;;
    --data) data="$2"; shift 2 ;;
    *) shift ;;
  esac
done
[[ "${data}" == *'license_key'* && "${data}" == *'hardware_fingerprint'* ]] || exit 10
[[ "${data}" != '{}' ]] || exit 11
printf 'curl data=%s\n' "${data}" >>"${FAKE_CALLS}"
printf '{"error":{"code":"%s"}}' "${FAKE_ERROR_CODE:-LICENSE_NOT_FOUND}" >"${output}"
printf '%s' "${FAKE_HTTP_STATUS:-404}"
FAKE_CURL

cat >"${fake_bin}/openssl" <<'FAKE_OPENSSL'
#!/usr/bin/env bash
set -euo pipefail
printf 'openssl %s\n' "$*" >>"${FAKE_CALLS}"
[[ "${1:-}" == 'pkey' ]] || exit 12
output=''
while (($#)); do
  if [[ "$1" == '-out' ]]; then output="$2"; shift 2; else shift; fi
done
printf '%s\n' '-----BEGIN PUBLIC KEY-----' 'test-public' '-----END PUBLIC KEY-----' >"${output}"
FAKE_OPENSSL
chmod +x "${fake_bin}/git" "${fake_bin}/npm" "${fake_bin}/doctl" \
  "${fake_bin}/curl" "${fake_bin}/openssl"

run_deploy() {
  PATH="${fake_bin}:${PATH}" \
  CRONOMETRIX_AIVEN_ADMIN_URL='admin-url-secret' \
  CRONOMETRIX_AIVEN_LICENSE_PASSWORD='runtime-password-secret' \
  CRONOMETRIX_AIVEN_CA_BASE64='ca-secret' \
  CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM='private-key-secret' \
    bash "${repo_root}/scripts/deploy-license-authority.sh"
}

success_output="${test_root}/success-output"
run_deploy >"${success_output}" 2>&1
grep -Fxq 'ACTIVATE_URL=https://functions.example/activate' "${success_output}"
grep -Fxq 'RENEW_URL=https://functions.example/renew' "${success_output}"
grep -Fxq 'npm --prefix do-functions run --silent provision:aiven -- --print-runtime-url' "${record}"
grep -Fxq 'doctl serverless namespaces create --label cronometrix --region nyc1' "${record}"
grep -Fxq 'doctl serverless connect cronometrix' "${record}"
grep -Fxq 'doctl serverless deploy do-functions --remote-build' "${record}"
grep -Fq 'openssl pkey -pubout -out' "${record}"
grep -Fq 'openssl pkey -pubin -in' "${record}"
[[ "$(grep -c '^curl data=' "${record}")" -eq 2 ]]
if grep -Eq 'runtime-secret|admin-url-secret|runtime-password-secret|ca-secret|private-key-secret' \
  "${success_output}"; then
  echo 'deployment output leaked injected credentials' >&2
  exit 1
fi

: >"${record}"
FAKE_NAMESPACE_LABELS=$'other\ncronometrix\ncronometrix-old' run_deploy >/dev/null 2>&1
if grep -Fq 'namespaces create' "${record}"; then
  echo 'exact existing namespace was unexpectedly recreated' >&2
  exit 1
fi
grep -Fxq 'doctl serverless connect cronometrix' "${record}"

expect_failure() {
  local output="$1"
  shift
  if "$@" >"${output}" 2>&1; then
    echo "expected deployment failure: ${output}" >&2
    exit 1
  fi
}

missing_output="${test_root}/missing-output"
expect_failure "${missing_output}" env \
  PATH="${fake_bin}:${PATH}" \
  CRONOMETRIX_AIVEN_LICENSE_PASSWORD='runtime-password-secret' \
  CRONOMETRIX_AIVEN_CA_BASE64='ca-secret' \
  CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM='private-key-secret' \
  bash "${repo_root}/scripts/deploy-license-authority.sh"

dirty_output="${test_root}/dirty-output"
FAKE_GIT_DIRTY=' M tracked-file' expect_failure "${dirty_output}" run_deploy

stale_output="${test_root}/stale-output"
FAKE_ORIGIN_SHA='2222222222222222222222222222222222222222' \
  expect_failure "${stale_output}" run_deploy

url_output="${test_root}/url-output"
FAKE_ACTIVATE_URL='http://not-secure.example/activate' \
  expect_failure "${url_output}" run_deploy

probe_output="${test_root}/probe-output"
FAKE_HTTP_STATUS=200 expect_failure "${probe_output}" run_deploy

echo 'license authority deployment tests passed'
