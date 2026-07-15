#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
API_DOCKERFILE="${ROOT_DIR}/deploy/Dockerfile.api"
WEB_DOCKERFILE="${ROOT_DIR}/deploy/Dockerfile.web"
GATEWAY_DOCKERFILE="${ROOT_DIR}/deploy/Dockerfile.gateway"

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

grep -Fq 'FROM rust:1.93-slim-bookworm@sha256:5b9332190bb3b9ece73b810cd1f1e9f06343b294ce184bcb067f0747d7d333ea AS builder' "${API_DOCKERFILE}" || fail "Rust builder is not digest-pinned"
grep -Fq 'FROM debian:bookworm-slim@sha256:60eac759739651111db372c07be67863818726f754804b8707c90979bda511df AS runner' "${API_DOCKERFILE}" || fail "Debian runner is not digest-pinned"
grep -Fq 'CMD ["/usr/local/bin/cronometrix"]' "${API_DOCKERFILE}" || fail "API does not execute cronometrix directly"
grep -Fq 'FROM node:24.15.0-alpine@sha256:d1b3b4da11eefd5941e7f0b9cf17783fc99d9c6fc34884a665f40a06dbdfc94f' "${WEB_DOCKERFILE}" || fail "Node base is not digest-pinned"
grep -Fq 'FROM nginx:1.27.5-alpine@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10' "${GATEWAY_DOCKERFILE}" || fail "Nginx base is not digest-pinned"

if grep -Eq 'seed_e2e|seed-reports-data|python3|entrypoint|CRONOMETRIX_AUTO_SEED' "${API_DOCKERFILE}"; then
    fail "production API Dockerfile includes demo/test tooling"
fi

[[ ! -e "${ROOT_DIR}/deploy/entrypoint.sh" ]] || fail "demo entrypoint still exists"

echo "PASS: production image contract"
