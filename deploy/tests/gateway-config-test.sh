#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG="${ROOT_DIR}/deploy/nginx.conf"

python3 - "${CONFIG}" <<'PY'
from pathlib import Path
import re
import sys

text = Path(sys.argv[1]).read_text()
assert "resolver 127.0.0.11" in text, "gateway must use Docker DNS"
assert "server api:3001 resolve;" in text, "API upstream must resolve dynamically"
assert "server web:3000 resolve;" in text, "web upstream must resolve dynamically"
exact = re.search(
    r"location\s+=\s+/api/v1/events/stream\s*\{(?P<body>.*?)\n\s*\}",
    text,
    re.DOTALL,
)
assert exact, "missing exact-match SSE location"
body = exact.group("body")
assert "access_log off;" in body, "SSE access logging must be disabled"
assert re.search(r"error_log\s+/dev/stderr\s+(crit|alert|emerg);", body), (
    "SSE error logging must suppress ordinary request/upstream failures"
)
assert "proxy_buffering off;" in body, "SSE proxy buffering must be disabled"
assert "proxy_cache off;" in body, "SSE proxy cache must be disabled"
assert text.index("location = /api/v1/events/stream") < text.index("location /api/"), (
    "exact SSE location must precede the generic API location"
)
PY

echo "PASS: gateway SSE logging and buffering boundary"
