#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_script="$repo_root/scripts/tests/test-ci-node-version-files.sh"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT

mkdir -p "$tmp_root/bin"
printf '#!/usr/bin/env bash\nexit 127\n' > "$tmp_root/bin/rg"
chmod +x "$tmp_root/bin/rg"

PATH="$tmp_root/bin:$PATH" bash "$test_script"
printf 'PASS: CI node-version-file tests run without ripgrep\n'
