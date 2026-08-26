#!/usr/bin/env bash
set -euo pipefail

if (($# != 1)) || [[ ! "$1" =~ ^cronometrix-license-[a-z0-9-]+$ ]]; then
  echo 'usage: create-license-secret.sh cronometrix-license-<name>' >&2
  exit 2
fi
destination_key="$1"
command -v secretctl >/dev/null || { echo 'secretctl is required' >&2; exit 1; }
command -v node >/dev/null || { echo 'node is required' >&2; exit 1; }
command -v mkfifo >/dev/null || { echo 'mkfifo is required' >&2; exit 1; }

vault_keys="$(secretctl list)"
if grep -Eq "(^|[[:space:]])${destination_key}($|[[:space:]])" <<<"${vault_keys}"; then
  echo "license secret already exists: ${destination_key}" >&2
  exit 1
fi
unset vault_keys

umask 077
temporary_root="$(mktemp -d)"
writer_pid=''
cleanup() {
  if [[ -n "${writer_pid}" ]] && kill -0 "${writer_pid}" 2>/dev/null; then
    kill "${writer_pid}" 2>/dev/null || true
    wait "${writer_pid}" 2>/dev/null || true
  fi
  rm -rf -- "${temporary_root}"
}
trap cleanup EXIT
import_fifo="${temporary_root}/license-secret.json"
mkfifo "${import_fifo}"

node - "${destination_key}" >"${import_fifo}" <<'NODE' &
const { randomInt } = require('node:crypto');
const destinationKey = process.argv[2];
const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
const groups = Array.from({ length: 4 }, () => {
  let group = '';
  for (let i = 0; i < 4; i += 1) group += alphabet[randomInt(alphabet.length)];
  return group;
});
process.stdout.write(JSON.stringify({ [destinationKey]: groups.join('-') }));
NODE
writer_pid=$!

secretctl import "${import_fifo}" --format=json --error >/dev/null
wait "${writer_pid}"
writer_pid=''

echo "license secret stored: ${destination_key}"
