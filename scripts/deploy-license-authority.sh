#!/usr/bin/env bash
set -euo pipefail
umask 077

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

: "${CRONOMETRIX_AIVEN_ADMIN_URL:?required}"
: "${CRONOMETRIX_AIVEN_LICENSE_PASSWORD:?required}"
: "${CRONOMETRIX_AIVEN_CA_BASE64:?required}"
: "${CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM:?required}"

git fetch origin main
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo 'deployment requires a clean tracked worktree' >&2
  exit 1
fi
SOURCE_SHA="$(git rev-parse HEAD)"
origin_main_sha="$(git rev-parse origin/main)"
if [[ "${SOURCE_SHA}" != "${origin_main_sha}" ]]; then
  echo 'deployment source must equal origin/main' >&2
  exit 1
fi
export SOURCE_SHA

bash scripts/verify-license-keypair.sh backend/src/license/pubkey.pem >/dev/null

namespace_labels="$(doctl serverless namespaces list --format Label --no-header)"
if ! grep -Fxq 'cronometrix' <<<"${namespace_labels}"; then
  doctl serverless namespaces create --label cronometrix --region nyc1 >/dev/null
fi
unset namespace_labels
doctl serverless connect cronometrix >/dev/null

DATABASE_URL="$(npm --prefix do-functions run --silent provision:aiven -- --print-runtime-url)"
if [[ "${DATABASE_URL}" != postgres://* && "${DATABASE_URL}" != postgresql://* ]]; then
  echo 'provisioner returned an invalid runtime database URL' >&2
  exit 1
fi
export DATABASE_URL
export DATABASE_CA_CERT_BASE64="${CRONOMETRIX_AIVEN_CA_BASE64}"
export LICENSE_PRIVATE_KEY="${CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM}"

doctl serverless deploy do-functions --remote-build >/dev/null
unset DATABASE_URL

activate_url="$(doctl serverless functions get licenses/activate --url)"
renew_url="$(doctl serverless functions get licenses/renew --url)"
for function_url in "${activate_url}" "${renew_url}"; do
  if [[ "${function_url}" != https://* || "${function_url}" =~ [[:space:]] ]]; then
    echo 'DigitalOcean returned an invalid Function URL' >&2
    exit 1
  fi
done

temporary_root="$(mktemp -d)"
cleanup() {
  rm -rf -- "${temporary_root}"
}
trap cleanup EXIT

probe_license_key="$(node -e '
  const { randomBytes } = require("node:crypto");
  const value = randomBytes(8).toString("hex").toUpperCase();
  process.stdout.write(value.match(/.{4}/g).join("-"));
')"
probe_body="$(node -e '
  process.stdout.write(JSON.stringify({
    license_key: process.argv[1],
    hardware_fingerprint: process.argv[2],
  }));
' "${probe_license_key}" "deployment-probe-${SOURCE_SHA}")"

probe_function() {
  local function_url="$1"
  local response_file="$2"
  local status
  status="$(curl --silent --show-error \
    --output "${response_file}" \
    --write-out '%{http_code}' \
    --request POST \
    --header 'Content-Type: application/json' \
    --data "${probe_body}" \
    "${function_url}")"
  if [[ "${status}" != '404' ]]; then
    echo 'license Function dependency probe returned an unexpected status' >&2
    return 1
  fi
  if ! node -e '
    const { readFileSync } = require("node:fs");
    const body = JSON.parse(readFileSync(process.argv[1], "utf8"));
    if (body?.error?.code !== "LICENSE_NOT_FOUND") process.exit(1);
  ' "${response_file}"; then
    echo 'license Function dependency probe returned an unexpected response' >&2
    return 1
  fi
}

probe_function "${activate_url}" "${temporary_root}/activate.json"
probe_function "${renew_url}" "${temporary_root}/renew.json"

unset DATABASE_CA_CERT_BASE64 LICENSE_PRIVATE_KEY
unset CRONOMETRIX_AIVEN_ADMIN_URL CRONOMETRIX_AIVEN_LICENSE_PASSWORD
unset CRONOMETRIX_AIVEN_CA_BASE64 CRONOMETRIX_LICENSE_PRIVATE_KEY_PEM

printf 'ACTIVATE_URL=%s\n' "${activate_url}"
printf 'RENEW_URL=%s\n' "${renew_url}"
