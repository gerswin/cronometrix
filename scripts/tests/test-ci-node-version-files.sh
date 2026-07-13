#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
guard="$repo_root/scripts/test-ci-node-version-files.sh"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT

write_repo() {
  local name="$1"
  local workflow="$2"
  local root="$tmp_root/$name"
  mkdir -p "$root/.github/workflows"
  printf '24.15.0\n' > "$root/.nvmrc"
  printf '%s\n' "$workflow" > "$root/.github/workflows/ci.yml"
  printf '%s\n' "$root"
}

run_guard() {
  local fixture_root="$1"
  local output="$2"
  set +e
  CRONOMETRIX_REPO_ROOT="$fixture_root" bash "$guard" >"$output" 2>&1
  local status=$?
  set -e
  printf '%s\n' "$status"
}

valid_root="$(write_repo valid 'name: CI
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with:
          node-version-file: .nvmrc
      - name: Named setup
        uses: actions/setup-node@v4
        with:
          node-version-file: ".nvmrc"')"
test "$(run_guard "$valid_root" "$tmp_root/valid.log")" -eq 0

misassociated_root="$(write_repo misassociated 'name: CI
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with:
          node-version-file: .nvmrc
      - uses: actions/setup-node@v4
      - name: Decoy field on another step
        node-version-file: .nvmrc')"
test "$(run_guard "$misassociated_root" "$tmp_root/misassociated.log")" -ne 0
rg -q 'setup-node step 2 has 0 node-version-file entries' "$tmp_root/misassociated.log"

duplicate_root="$(write_repo duplicate 'name: CI
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with:
          node-version-file: .nvmrc
          node-version-file: .nvmrc')"
test "$(run_guard "$duplicate_root" "$tmp_root/duplicate.log")" -ne 0
rg -q 'setup-node step 1 has 2 node-version-file entries' "$tmp_root/duplicate.log"

missing_root="$(write_repo missing 'name: CI
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with:
          node-version-file: frontend/.nvmrc')"
test "$(run_guard "$missing_root" "$tmp_root/missing.log")" -ne 0
rg -q 'references missing node-version-file: frontend/.nvmrc' "$tmp_root/missing.log"

no_setup_root="$(write_repo no-setup 'name: CI
jobs:
  test:
    steps:
      - run: node --version')"
test "$(run_guard "$no_setup_root" "$tmp_root/no-setup.log")" -ne 0
rg -q 'has no setup-node steps' "$tmp_root/no-setup.log"

bash "$guard"
printf 'PASS: CI node-version-file association guard\n'
