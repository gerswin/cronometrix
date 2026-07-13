#!/usr/bin/env bash
set -euo pipefail

repo_root="${CRONOMETRIX_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

python3 - "$repo_root" <<'PY'
from pathlib import Path
import re
import sys

repo_root = Path(sys.argv[1])
paths = {
    "Makefile": repo_root / "Makefile",
    "frontend/playwright.config.ts": repo_root / "frontend/playwright.config.ts",
    ".github/workflows/ci.yml": repo_root / ".github/workflows/ci.yml",
}
errors: list[str] = []

for relative, path in paths.items():
    if not path.is_file():
        errors.append(f"FAIL: missing E2E harness configuration file: {relative}")

if errors:
    print("\n".join(errors), file=sys.stderr)
    raise SystemExit(1)

makefile = paths["Makefile"].read_text()
playwright = paths["frontend/playwright.config.ts"].read_text()
workflow = paths[".github/workflows/ci.yml"].read_text()


def require(condition: bool, message: str) -> None:
    if not condition:
        errors.append(f"FAIL: {message}")


api_url_assignments = re.findall(
    r"(?m)^NEXT_PUBLIC_API_URL[ \t]*(?:\?|:|\+)?=[^\n]*$", makefile
)
require(
    api_url_assignments == ["NEXT_PUBLIC_API_URL ?= http://localhost:4001"],
    "Makefile must define exactly `NEXT_PUBLIC_API_URL ?= http://localhost:4001`",
)

target_match = re.search(
    r"(?m)^e2e-build:[^\n]*\n(?P<body>(?:^\t[^\n]*(?:\n|\Z))*)", makefile
)
require(target_match is not None, "Makefile must define the e2e-build target")
if target_match is not None:
    build_env = re.findall(
        r'(?m)^\tcd frontend && NEXT_PUBLIC_API_URL="\$\(NEXT_PUBLIC_API_URL\)" npm run build\s*$',
        target_match.group("body"),
    )
    require(
        len(build_env) == 1,
        "e2e-build must pass the overridable NEXT_PUBLIC_API_URL Make variable to npm run build",
    )

web_server_marker = "  webServer: [\n"
backend_boundary = "\n    },\n    {"
backend_server = ""
if web_server_marker in playwright:
    web_servers = playwright.split(web_server_marker, 1)[1]
    if backend_boundary in web_servers:
        backend_server = web_servers.split(backend_boundary, 1)[0]
require(
    bool(backend_server),
    "frontend/playwright.config.ts must expose the first webServer as the backend server",
)
if backend_server:
    expected_backend_env = {
        "CORS_ALLOWED_ORIGINS": '        CORS_ALLOWED_ORIGINS: "http://localhost:3001",',
        "COOKIE_SECURE": '        COOKIE_SECURE: "false",',
    }
    for key, expected_line in expected_backend_env.items():
        assignments = re.findall(rf"(?m)^[ \t]*{key}:.*$", backend_server)
        require(
            assignments == [expected_line],
            "frontend/playwright.config.ts backend webServer env must set exactly "
            f"`{expected_line.strip()}`",
        )

job_match = re.search(
    r"(?ms)^  e2e-tests:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)", workflow
)
require(job_match is not None, ".github/workflows/ci.yml must define the e2e-tests job")
if job_match is not None:
    e2e_job = job_match.group("body")
    shared_builds = re.findall(r"(?m)^\s+run:\s*make e2e-build\s*$", e2e_job)
    require(
        len(shared_builds) == 1,
        "the E2E Tests job must invoke exactly one `make e2e-build` shared build",
    )
    require(
        "npm run build" not in e2e_job,
        "the E2E Tests job must not duplicate the frontend `npm run build` sequence",
    )
    require(
        "cargo build" not in e2e_job,
        "the E2E Tests job must not duplicate backend `cargo build` commands",
    )

if errors:
    print("\n".join(errors), file=sys.stderr)
    raise SystemExit(1)

print("PASS: E2E harness configuration contracts")
PY
