#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo 'usage: prepare-license-secrets.sh --ca-file PATH --public-key-out PATH [--rotate]' >&2
  exit 2
}

ca_file=''
public_key_out=''
rotate=false
while (($#)); do
  case "$1" in
    --ca-file)
      (($# >= 2)) || usage
      ca_file="$2"
      shift 2
      ;;
    --public-key-out)
      (($# >= 2)) || usage
      public_key_out="$2"
      shift 2
      ;;
    --rotate)
      rotate=true
      shift
      ;;
    *)
      usage
      ;;
  esac
done

[[ -n "${ca_file}" && -f "${ca_file}" && -n "${public_key_out}" ]] || usage
command -v secretctl >/dev/null || { echo 'secretctl is required' >&2; exit 1; }
command -v openssl >/dev/null || { echo 'openssl is required' >&2; exit 1; }
command -v node >/dev/null || { echo 'node is required' >&2; exit 1; }
command -v mkfifo >/dev/null || { echo 'mkfifo is required' >&2; exit 1; }
openssl x509 -in "${ca_file}" -noout >/dev/null 2>&1 \
  || { echo 'Aiven CA certificate is invalid' >&2; exit 1; }

public_parent="$(dirname "${public_key_out}")"
[[ -d "${public_parent}" && -w "${public_parent}" ]] \
  || { echo 'public key destination is not writable' >&2; exit 1; }
if [[ -e "${public_key_out}" && "${rotate}" != true ]]; then
  echo 'public key already exists; use --rotate to replace it' >&2
  exit 1
fi

vault_keys="$(secretctl list)"
required_keys=(
  cronometrix-aiven-license-password
  cronometrix-aiven-ca-base64
  cronometrix-license-private-key-pem
)
if [[ "${rotate}" != true ]]; then
  for key in "${required_keys[@]}"; do
    if grep -Eq "(^|[[:space:]])${key}($|[[:space:]])" <<<"${vault_keys}"; then
      echo "vault credential already exists: ${key}; use --rotate to replace it" >&2
      exit 1
    fi
  done
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
private_key="${temporary_root}/license-private.pem"
public_key="${temporary_root}/license-public.pem"
import_fifo="${temporary_root}/license-secrets.json"

openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 \
  -out "${private_key}" >/dev/null 2>&1
openssl pkey -in "${private_key}" -pubout -out "${public_key}" >/dev/null 2>&1

mkfifo "${import_fifo}"
node - "${ca_file}" "${private_key}" >"${import_fifo}" <<'NODE' &
const { randomInt } = require('node:crypto');
const fs = require('node:fs');
const [caPath, privateKeyPath] = process.argv.slice(2);
const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
let runtimePassword = '';
for (let i = 0; i < 40; i += 1) {
  runtimePassword += alphabet[randomInt(alphabet.length)];
}
process.stdout.write(JSON.stringify({
  'cronometrix-aiven-license-password': runtimePassword,
  'cronometrix-aiven-ca-base64': fs.readFileSync(caPath).toString('base64'),
  'cronometrix-license-private-key-pem': fs.readFileSync(privateKeyPath, 'utf8'),
}));
NODE
writer_pid=$!

conflict_flag='--error'
if [[ "${rotate}" == true ]]; then
  conflict_flag='--overwrite'
fi
secretctl import "${import_fifo}" --format=json "${conflict_flag}" >/dev/null
wait "${writer_pid}"
writer_pid=''

cp "${public_key}" "${public_key_out}"
chmod 0644 "${public_key_out}"
echo 'license secrets prepared'
