#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workflow_rel=".github/workflows/ci.yml"
workflow="$repo_root/$workflow_rel"

setup_count="$(
  grep -Ec '^[[:space:]]*-[[:space:]]+uses:[[:space:]]+actions/setup-node@' \
    "$workflow" || true
)"
version_file_count="$(
  grep -Ec '^[[:space:]]*node-version-file:' "$workflow" || true
)"

if [[ "$setup_count" -eq 0 || "$setup_count" -ne "$version_file_count" ]]; then
  printf 'FAIL: %s has %s setup-node steps but %s node-version-file entries\n' \
    "$workflow_rel" "$setup_count" "$version_file_count" >&2
  exit 1
fi

failed=0
while IFS= read -r referenced_path; do
  if [[ ! -f "$repo_root/$referenced_path" ]]; then
    printf 'FAIL: %s references missing node-version-file: %s\n' \
      "$workflow_rel" "$referenced_path" >&2
    failed=1
  fi
done < <(
  awk '$1 == "node-version-file:" { print $2 }' "$workflow" |
    tr -d "\"'"
)

exit "$failed"
