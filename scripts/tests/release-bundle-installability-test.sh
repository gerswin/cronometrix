#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

SOURCE_SHA=4101bb58f93dd5b0a77cb331e8684174be5d604b \
RELEASE_VERSION=sha-4101bb58f93d \
API_IMAGE=ghcr.io/example/api@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
WEB_IMAGE=ghcr.io/example/web@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb \
GATEWAY_IMAGE=ghcr.io/example/gateway@sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc \
CLOUDFLARED_IMAGE=cloudflare/cloudflared@sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd \
CRONOMETRIX_RELEASE_OUTPUT_DIR="${TMP_DIR}/output" \
bash "${ROOT_DIR}/scripts/assemble-release-bundle.sh"

archive="$(find "${TMP_DIR}/output/dist" -name '*.tar.gz' -type f)"
expected=$'SHA256SUMS\ndocker-compose.yml\ninstall.sh\nlib/evidence-backup.sh\nnginx.conf\nrelease-manifest.env'
actual="$(tar -tzf "${archive}" | sort)"
[[ "${actual}" == "${expected}" ]]

mkdir "${TMP_DIR}/extracted"
tar -xzf "${archive}" -C "${TMP_DIR}/extracted"
(cd "${TMP_DIR}/extracted" && sha256sum --strict -c SHA256SUMS)

# Force a deterministic non-root preflight result even when this test itself
# runs as root inside a Linux development or CI container.
mkdir "${TMP_DIR}/fake-bin"
cat > "${TMP_DIR}/fake-bin/id" <<'SH'
#!/usr/bin/env bash
if [[ "${1:-}" == "-u" ]]; then
    echo 1000
    exit 0
fi
exec /usr/bin/id "$@"
SH
chmod 0755 "${TMP_DIR}/fake-bin/id"

set +e
output="$(PATH="${TMP_DIR}/fake-bin:${PATH}" bash "${TMP_DIR}/extracted/install.sh" 2>&1)"
status=$?
set -e
[[ "${status}" -ne 0 ]]
[[ "${output}" != *'evidence-backup.sh: No such file or directory'* ]]
[[ "${output}" == *'must run as root'* ]]

# Mutation probe: a tampered helper must be rejected before Bash sources it.
marker="${TMP_DIR}/helper-executed"
printf 'touch %q\n' "${marker}" | cat - "${TMP_DIR}/extracted/lib/evidence-backup.sh" \
  > "${TMP_DIR}/tampered-helper"
mv "${TMP_DIR}/tampered-helper" "${TMP_DIR}/extracted/lib/evidence-backup.sh"
set +e
tampered_output="$(bash "${TMP_DIR}/extracted/install.sh" 2>&1)"
tampered_status=$?
set -e
[[ "${tampered_status}" -ne 0 ]]
[[ ! -e "${marker}" ]]
[[ "${tampered_output}" == *'bundle checksum verification failed'* ]]

echo "PASS: assembled release bundle is complete and verifies helpers before execution"
