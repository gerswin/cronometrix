#!/usr/bin/env bash
set -euo pipefail

if (($# != 1)) || [[ ! "$1" =~ ^cronometrix-license-[a-z0-9-]+$ ]]; then
  echo 'usage: create-license-secret.sh cronometrix-license-<name>' >&2
  exit 2
fi
destination_key="$1"
command -v secretctl >/dev/null || { echo 'secretctl is required' >&2; exit 1; }
command -v node >/dev/null || { echo 'node is required' >&2; exit 1; }

vault_keys="$(secretctl list)"
if grep -Eq "(^|[[:space:]])${destination_key}($|[[:space:]])" <<<"${vault_keys}"; then
  echo "license secret already exists: ${destination_key}" >&2
  exit 1
fi
unset vault_keys

node -e '
  const { randomInt } = require("node:crypto");
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  const groups = Array.from({ length: 4 }, () => {
    let group = "";
    for (let i = 0; i < 4; i += 1) group += alphabet[randomInt(36)];
    return group;
  });
  process.stdout.write(groups.join("-"));
' | secretctl set "${destination_key}" >/dev/null

echo "license secret stored: ${destination_key}"
