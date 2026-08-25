# Release Bundle Helper and LAN Installation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish an installable immutable Cronometrix bundle and deploy that exact release to `192.168.1.239`.

**Architecture:** Keep evidence backup/restore logic in its existing deployment adapter (`deploy/lib/evidence-backup.sh`) and make the release artifact carry that dependency explicitly. Centralize bundle assembly in one tested script consumed by GitHub Actions, verify the complete file set before sourcing the helper, then install only the CI-approved digest-pinned release on the target host.

**Tech Stack:** Bash, GitHub Actions, Docker Compose v2, GHCR, SSH, Graphify CLI.

**Spec:** `deploy/INSTALL.md`

## Global Constraints

- Preserve the existing hexagonal application boundaries; all changes stay in deployment infrastructure.
- Release images remain immutable and digest-pinned; never introduce `latest` tags.
- The bundle must contain exactly six regular files: `install.sh`, `docker-compose.yml`, `release-manifest.env`, `nginx.conf`, `lib/evidence-backup.sh`, and `SHA256SUMS`.
- The internal checksum file must cover the other five files, including `lib/evidence-backup.sh`.
- Verify the bundle before sourcing `lib/evidence-backup.sh`.
- The supported target remains Linux/amd64 with Docker >= 24.0.0, Compose >= 2.24.0, and at least 2 GiB free.
- Credentials must come from `secretctl` or silent installer input and must never be written to repository files, command arguments, logs, or the release manifest.
- Preserve transactional backup/rollback of SQLite plus `enrollments`, `events`, `leaves`, and `overrides`.

---

### Task 1: Add a behavioral regression test for the published bundle

**Files:**
- Create: `scripts/tests/release-bundle-installability-test.sh`
- Test: `scripts/tests/release-bundle-installability-test.sh`

**Interfaces:**
- Consumes: release assembly entry point `scripts/assemble-release-bundle.sh` and the existing deployment files.
- Produces: a test that proves the assembled archive has the exact safe member set, valid checksums, and an installer that reaches preflight without a missing-helper error.

- [ ] **Step 1: Write the failing test**

```bash
#!/usr/bin/env bash
set -euo pipefail

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
```

- [ ] **Step 2: Run the test and verify RED**

Run: `bash scripts/tests/release-bundle-installability-test.sh`

Expected: FAIL because `scripts/assemble-release-bundle.sh` does not exist. After the assembler is added, the same test must continue to fail until the installer verifies the helper checksum before sourcing it.

### Task 2: Centralize and repair immutable bundle assembly

**Files:**
- Create: `scripts/assemble-release-bundle.sh`
- Modify: `.github/workflows/release.yml`
- Modify: `Makefile`
- Modify: `deploy/install.sh`
- Modify: `deploy/INSTALL.md`
- Modify: `scripts/tests/release-workflow-test.py`
- Test: `scripts/tests/release-bundle-installability-test.sh`
- Test: `deploy/tests/install-static-test.sh`
- Test: `deploy/tests/backup-restore-test.sh`
- Test: `scripts/tests/release-workflow-test.py`

**Interfaces:**
- Consumes: `SOURCE_SHA`, `RELEASE_VERSION`, `API_IMAGE`, `WEB_IMAGE`, `GATEWAY_IMAGE`, and `CLOUDFLARED_IMAGE` environment variables.
- Produces: `dist/cronometrix-${RELEASE_VERSION}-${SOURCE_SHA:0:12}.tar.gz` plus its external `.sha256`; the workflow uploads those files unchanged.

- [ ] **Step 1: Implement the minimal assembler**

```bash
#!/usr/bin/env bash
set -Eeuo pipefail

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
[[ ! -e "${BUNDLE_DIR}" && ! -e "${DIST_DIR}" ]] || {
  printf 'release output must start empty: %s\n' "${OUTPUT_ROOT}" >&2
  exit 1
}
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
[[ "$(tar -tzf "${DIST_DIR}/${ARCHIVE}" | sort)" == "${expected}" ]]
(
  cd "${DIST_DIR}"
  sha256sum "${ARCHIVE}" > "${ARCHIVE}.sha256"
  sha256sum --strict -c "${ARCHIVE}.sha256"
)
```

- [ ] **Step 2: Make the installer verify before loading the helper**

```bash
load_evidence_backup_helpers() {
    # shellcheck source=deploy/lib/evidence-backup.sh
    source "${BUNDLE_DIR}/lib/evidence-backup.sh"
}

main() {
    [[ "$#" -eq 0 ]] || die "positional arguments are not accepted"
    verify_bundle
    verify_release_manifest "${BUNDLE_MANIFEST}" >/dev/null
    load_evidence_backup_helpers
    preflight
    read_inputs
    prepare_directories_and_secrets
    login_ghcr
    backup_existing_release
    TRANSACTION_ACTIVE=1
}
```

Update `verify_bundle` to require the real `lib/` directory, require exactly the six regular files, reject links/extras, and require `SHA256SUMS` to cover exactly the five payload files.

- [ ] **Step 3: Replace inline workflow assembly with the tested script**

```yaml
- name: Assemble and verify private bundle
  shell: bash
  run: |
    set -euo pipefail
    export SOURCE_SHA RELEASE_VERSION API_IMAGE WEB_IMAGE GATEWAY_IMAGE
    export CLOUDFLARED_IMAGE='cloudflare/cloudflared:2026.3.0@sha256:6b599ca3e974349ead3286d178da61d291961182ec3fe9c505e1dd02c8ac31b0'
    bash scripts/assemble-release-bundle.sh
```

- [ ] **Step 4: Update the workflow contract test and operator documentation**

Assert that the parsed workflow invokes `scripts/assemble-release-bundle.sh`; document the exact six-file archive and checksum coverage in `deploy/INSTALL.md`.

- [ ] **Step 5: Run focused verification and verify GREEN**

Run:

```bash
bash scripts/tests/release-bundle-installability-test.sh
bash deploy/tests/install-static-test.sh
bash deploy/tests/backup-restore-test.sh
python3 scripts/tests/release-workflow-test.py
bash deploy/tests/compose-smoke.sh
git diff --check
```

Expected: all commands PASS and the assembled installer reaches platform/root preflight rather than failing on the missing helper.

- [ ] **Step 6: Commit the repair**

```bash
git add scripts/assemble-release-bundle.sh scripts/tests/release-bundle-installability-test.sh \
  .github/workflows/release.yml deploy/install.sh deploy/INSTALL.md \
  scripts/tests/release-workflow-test.py docs/superpowers/plans/2026-08-25-release-bundle-helper-and-lan-install.md
git commit -m "fix(release): include installer backup helper"
```

### Task 3: Ship the corrected release

**Files:**
- No additional source files.

**Interfaces:**
- Consumes: the committed fix branch and GitHub Actions required checks.
- Produces: merged `main` plus a successful immutable `codex/release-build-${MAIN_SHA}` artifact.

- [ ] **Step 1: Push the branch and open a PR to `main`**

```bash
git push -u origin fix/release-bundle-evidence-helper
gh pr create --base main --head fix/release-bundle-evidence-helper \
  --title "fix(release): include installer backup helper" \
  --body-file /tmp/cronometrix-release-bundle-pr.md
```

- [ ] **Step 2: Wait for Backend Coverage, Frontend Coverage, E2E Tests, Container Smoke, and Secret Scan**

Run: `gh pr checks --watch --fail-fast "$(gh pr view --json number --jq .number)"`

- [ ] **Step 3: Investigate and repair any failed check before continuing**

Resolve the failed run with `FAILED_RUN_ID="$(gh run list --branch fix/release-bundle-evidence-helper --status failure --limit 1 --json databaseId --jq '.[0].databaseId')"` and `gh run view "${FAILED_RUN_ID}" --log-failed`; apply TDD to any source defect, push, and repeat Step 2.

- [ ] **Step 4: Merge the PR and verify post-merge CI on the exact `main` SHA**

Run: `gh pr merge --squash --delete-branch "$(gh pr view --json number --jq .number)"`, then wait for the `push` workflow associated with `git rev-parse origin/main`.

- [ ] **Step 5: Push `codex/release-build-${MAIN_SHA}` at that exact SHA and wait for the Release workflow**

Run: `git push origin "$(git rev-parse origin/main):refs/heads/codex/release-build-$(git rev-parse origin/main)"`, then monitor both Release and CI runs for that SHA.

- [ ] **Step 6: Download the new private artifact and independently verify its external checksum, six-member archive, and internal checksums**

Set `MAIN_SHA="$(git rev-parse origin/main)"`, resolve `RELEASE_RUN_ID` with `gh run list --workflow Release --commit "${MAIN_SHA}" --limit 1 --json databaseId --jq '.[0].databaseId'`, create `DOWNLOAD_DIR="$(mktemp -d)"`, then run `gh run download "${RELEASE_RUN_ID}" -n "cronometrix-private-release-${MAIN_SHA}" -D "${DOWNLOAD_DIR}"`. Validate the external digest from inside that directory, inspect `tar -tzf`, extract into another `mktemp -d` directory, and run `sha256sum --strict -c SHA256SUMS` there.

### Task 4: Install the verified release on `192.168.1.239`

**Files:**
- Remote installation root: `/opt/cronometrix`
- Temporary remote staging directory: a root-owned directory created with `mktemp -d`

**Interfaces:**
- Consumes: authenticated SSH/sudo access, verified release archive, GHCR read-only token, license key, client slug, admin password, Cloudflare tunnel token, and DigitalOcean activation/renewal URLs.
- Produces: a healthy Docker Compose deployment whose local gateway responds at `http://127.0.0.1:8080` and whose public hostname is derived from `CLIENT_SLUG` as `https://${CLIENT_SLUG}.cronometrix.com`.

- [ ] **Step 1: Retrieve credentials from unlocked `secretctl` and verify SSH/sudo without exposing values**

Run: `secretctl list`, then `ssh -o BatchMode=yes -o ConnectTimeout=8 gerswin@192.168.1.239 'id -un; sudo -n true'`. Stop without transferring anything if either credential boundary remains locked.

- [ ] **Step 2: Check Linux/amd64, Docker/Compose versions, free space, current `/opt/cronometrix` state, ports, and production bypass flags**

Run the read-only remote checks `uname -srm`, `docker version`, `docker compose version`, `df -Pk /opt`, `sudo ss -lntp`, and inspect only the names (not values) of keys in `/opt/cronometrix/.env`. Abort if `CRONOMETRIX_E2E` or `CRONOMETRIX_LICENSE_BYPASS` is present.

- [ ] **Step 3: Transfer the archive and checksum over SCP into an empty root-owned staging directory**

Create the staging path with `ssh gerswin@192.168.1.239 'sudo mktemp -d /var/tmp/cronometrix-release.XXXXXX'`, then copy the two verified files with `scp` and move them into that directory using `sudo install`.

- [ ] **Step 4: Verify the external checksum and safe tar member set on the target before extraction**

From the staging directory run `sha256sum --strict -c *.tar.gz.sha256`, compare sorted `tar -tzf` output with the six literal member names in the global constraint, and extract only after both checks pass.

- [ ] **Step 5: Run `sudo bash install.sh` with secrets supplied through silent input or `secretctl run`, never through positional arguments**

Store the validated remote extraction path in `REMOTE_RELEASE_DIR`, then run `ssh -tt gerswin@192.168.1.239 "cd '${REMOTE_RELEASE_DIR}' && sudo bash install.sh"`; answer its silent prompts using values retrieved from the unlocked vault without echoing or logging them.

- [ ] **Step 6: Verify Compose health, `http://127.0.0.1:8080/api/v1/health`, setup/licensing state, cloudflared state, image digests, file permissions, and absence of test bypass flags**

Run `sudo docker compose --project-directory /opt/cronometrix --env-file /opt/cronometrix/.env --env-file /opt/cronometrix/release-manifest.env -f /opt/cronometrix/docker-compose.yml ps`, the local health/setup curls, `docker inspect` image IDs, `stat` on `.env`, `.docker`, and `data`, and a key-name-only bypass scan.

- [ ] **Step 7: Verify the public hostname and record the installed release SHA without printing credentials**

Read `SOURCE_SHA`, `RELEASE_VERSION`, and `CLIENT_SLUG` from the root-readable manifest/environment files, verify `https://${CLIENT_SLUG}.cronometrix.com/api/v1/health`, and report only the SHA, version, health state, and public URL.
