#!/usr/bin/env bash
set -euo pipefail

repo_root="${CRONOMETRIX_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

python3 - "$repo_root" <<'PY'
from pathlib import Path
import re
import sys

EXPECTED_API_DEFAULT = "NEXT_PUBLIC_API_URL ?= http://localhost:4001"
EXPECTED_BUILD_LINE = (
    '\tcd frontend && NEXT_PUBLIC_API_URL="$(NEXT_PUBLIC_API_URL)" npm run build'
)
EXPECTED_E2E_RECIPE = [
    "\tcd backend && cargo build --release --bin cronometrix",
    "\tcd backend && cargo build --release --bin mock_hikvision --features mock-hikvision",
    "\tcd backend && cargo build --release --bin seed_e2e --features seed-e2e",
    EXPECTED_BUILD_LINE,
]
EXPECTED_CI_BUILD = 'make -C "$GITHUB_WORKSPACE" e2e-build'
EXPECTED_BACKEND_COMMAND = [
    "      command: isE2ERelease",
    '        ? "../backend/target/release/cronometrix"',
    '        : "../backend/target/debug/cronometrix",',
]
EXPECTED_BACKEND_URL = '      url: "http://127.0.0.1:4001/api/v1/health",'
EXPECTED_BACKEND_ENV = {
    "CORS_ALLOWED_ORIGINS": '        CORS_ALLOWED_ORIGINS: "http://localhost:3001",',
    "COOKIE_SECURE": '        COOKIE_SECURE: "false",',
}


def indentation(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def yaml_field(line: str) -> tuple[str, str] | None:
    value = line.strip()
    if value.startswith("- "):
        value = value[2:].lstrip()
    match = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_-]*):(?:[ \t]*(.*))?", value)
    if match is None:
        return None
    return match.group(1), match.group(2) or ""


def make_rule_targets(line: str) -> list[str]:
    if not line or line[0].isspace() or line.startswith("#"):
        return []
    match = re.match(r"^([^:=#]+?)(?:::|:)(?=[ \t]|$)", line)
    if match is None:
        return []
    return match.group(1).split()


def validate_makefile(makefile: str) -> list[str]:
    errors: list[str] = []
    assignments = re.findall(
        r"(?m)^NEXT_PUBLIC_API_URL[ \t]*(?:\?|:|\+)?=[^\n]*$", makefile
    )
    if assignments != [EXPECTED_API_DEFAULT]:
        errors.append(
            "Makefile must define exactly "
            "`NEXT_PUBLIC_API_URL ?= http://localhost:4001`"
        )

    lines = makefile.splitlines()
    rule_indexes = [
        index
        for index, line in enumerate(lines)
        if "e2e-build" in make_rule_targets(line)
    ]
    if len(rule_indexes) != 1:
        errors.append(
            f"Makefile must define exactly one e2e-build rule; found {len(rule_indexes)}"
        )
        return errors

    recipe: list[str] = []
    index = rule_indexes[0] + 1
    while index < len(lines):
        line = lines[index]
        if line.startswith("\t"):
            recipe.append(line)
            index += 1
            continue
        if not line.strip() or line.lstrip().startswith("#"):
            index += 1
            continue
        break

    effective_recipe = [
        line
        for line in recipe
        if line[1:].strip() and not line[1:].lstrip().startswith("#")
    ]
    npm_build_count = sum(line.count("npm run build") for line in effective_recipe)
    npm_build_lines = [line for line in effective_recipe if "npm run build" in line]
    if npm_build_count != 1:
        errors.append(
            "e2e-build must contain exactly one effective `npm run build`; "
            f"found {npm_build_count}"
        )
    if npm_build_lines != [EXPECTED_BUILD_LINE]:
        errors.append(
            "the sole e2e-build frontend build must be exactly "
            "`cd frontend && NEXT_PUBLIC_API_URL=\"$(NEXT_PUBLIC_API_URL)\" npm run build`"
        )
    if effective_recipe != EXPECTED_E2E_RECIPE:
        errors.append(
            "the entire effective e2e-build recipe must be exactly the four permitted "
            "backend/frontend build commands"
        )
    return errors


def validate_playwright(playwright: str) -> list[str]:
    errors: list[str] = []
    lines = playwright.splitlines()
    web_server_indexes = [
        index
        for index, line in enumerate(lines)
        if indentation(line) == 2 and line.strip() == "webServer: ["
    ]
    if len(web_server_indexes) != 1:
        errors.append(
            "frontend/playwright.config.ts must define exactly one top-level webServer array"
        )
        return errors

    array_index = web_server_indexes[0]
    first_server_start = next(
        (
            index
            for index in range(array_index + 1, len(lines))
            if lines[index].strip()
        ),
        None,
    )
    if (
        first_server_start is None
        or indentation(lines[first_server_start]) != 4
        or lines[first_server_start].strip() != "{"
    ):
        errors.append(
            "frontend/playwright.config.ts must expose an object as the first webServer"
        )
        return errors

    first_server_end = next(
        (
            index
            for index in range(first_server_start + 1, len(lines))
            if indentation(lines[index]) == 4 and lines[index].strip() == "},"
        ),
        None,
    )
    if first_server_end is None:
        errors.append(
            "frontend/playwright.config.ts first webServer object is not indentation-delimited"
        )
        return errors

    direct_command_indexes = [
        index
        for index in range(first_server_start + 1, first_server_end)
        if indentation(lines[index]) == 6
        and re.match(r"^command[ \t]*:", lines[index].strip()) is not None
    ]
    command_is_exact = (
        len(direct_command_indexes) == 1
        and lines[
            direct_command_indexes[0] : direct_command_indexes[0]
            + len(EXPECTED_BACKEND_COMMAND)
        ]
        == EXPECTED_BACKEND_COMMAND
    )
    if not command_is_exact:
        errors.append(
            "frontend/playwright.config.ts first webServer must use the exact "
            "release/debug cronometrix command"
        )

    direct_urls = [
        lines[index]
        for index in range(first_server_start + 1, first_server_end)
        if indentation(lines[index]) == 6
        and re.match(r"^url[ \t]*:", lines[index].strip()) is not None
    ]
    if direct_urls != [EXPECTED_BACKEND_URL]:
        errors.append(
            "frontend/playwright.config.ts first webServer must use the exact Axum health URL"
        )

    env_starts = [
        index
        for index in range(first_server_start + 1, first_server_end)
        if indentation(lines[index]) == 6 and lines[index].strip() == "env: {"
    ]
    if len(env_starts) != 1:
        errors.append(
            "frontend/playwright.config.ts first webServer must define exactly one direct env block"
        )
        return errors

    env_start = env_starts[0]
    env_end = next(
        (
            index
            for index in range(env_start + 1, first_server_end)
            if lines[index].strip() and indentation(lines[index]) <= 6
        ),
        None,
    )
    if (
        env_end is None
        or indentation(lines[env_end]) != 6
        or lines[env_end].strip() != "},"
    ):
        errors.append(
            "frontend/playwright.config.ts first webServer env block is not indentation-delimited"
        )
        return errors

    env_indexes = set(range(env_start + 1, env_end))
    required_assignment_indexes: list[int] = []
    for key, expected_line in EXPECTED_BACKEND_ENV.items():
        assignment_indexes = [
            index
            for index in range(first_server_start + 1, first_server_end)
            if re.match(rf"^[ \t]*{key}[ \t]*:", lines[index]) is not None
        ]
        inside_assignments = [
            lines[index] for index in assignment_indexes if index in env_indexes
        ]
        inside_assignment_indexes = [
            index for index in assignment_indexes if index in env_indexes
        ]
        outside_assignments = [
            index for index in assignment_indexes if index not in env_indexes
        ]
        if inside_assignments != [expected_line]:
            errors.append(
                "frontend/playwright.config.ts first webServer env must set exactly "
                f"`{expected_line.strip()}`"
            )
        else:
            required_assignment_indexes.extend(inside_assignment_indexes)
        if outside_assignments:
            errors.append(
                f"frontend/playwright.config.ts first webServer must not assign {key} outside env"
            )
    if len(required_assignment_indexes) == len(EXPECTED_BACKEND_ENV):
        first_required_assignment = min(required_assignment_indexes)
        late_spreads = [
            index
            for index in env_indexes
            if index > first_required_assignment
            and lines[index].strip().startswith("...")
        ]
        if late_spreads:
            errors.append(
                "frontend/playwright.config.ts first webServer env must not contain a "
                "spread after CORS/cookie assignments"
            )
    return errors


def validate_workflow(workflow: str) -> list[str]:
    errors: list[str] = []
    lines = workflow.splitlines()
    job_indexes = [
        index
        for index, line in enumerate(lines)
        if indentation(line) == 2 and line.strip() == "e2e-tests:"
    ]
    if len(job_indexes) != 1:
        errors.append(
            f".github/workflows/ci.yml must define exactly one e2e-tests job; found {len(job_indexes)}"
        )
        return errors

    job_start = job_indexes[0]
    job_end = next(
        (
            index
            for index in range(job_start + 1, len(lines))
            if lines[index].strip()
            and not lines[index].lstrip().startswith("#")
            and indentation(lines[index]) <= 2
        ),
        len(lines),
    )

    job_if_fields = [
        index
        for index in range(job_start + 1, job_end)
        if indentation(lines[index]) == 4
        and yaml_field(lines[index]) is not None
        and yaml_field(lines[index])[0] == "if"
    ]
    if job_if_fields:
        errors.append("the e2e-tests job must not define `if:` and risk being disabled")

    steps_indexes = [
        index
        for index in range(job_start + 1, job_end)
        if indentation(lines[index]) == 4 and lines[index].strip() == "steps:"
    ]
    if len(steps_indexes) != 1:
        errors.append("the e2e-tests job must define exactly one direct steps block")
        return errors

    steps_start = steps_indexes[0]
    steps_end = next(
        (
            index
            for index in range(steps_start + 1, job_end)
            if lines[index].strip()
            and not lines[index].lstrip().startswith("#")
            and indentation(lines[index]) <= 4
        ),
        job_end,
    )
    step_starts = [
        index
        for index in range(steps_start + 1, steps_end)
        if indentation(lines[index]) == 6 and lines[index].strip().startswith("- ")
    ]

    direct_build_runs: list[tuple[int, list[tuple[str, str]]]] = []
    direct_build_run_indexes: list[int] = []
    for position, step_start in enumerate(step_starts):
        step_end = (
            step_starts[position + 1]
            if position + 1 < len(step_starts)
            else steps_end
        )
        direct_fields: list[tuple[str, str]] = []
        for index in range(step_start, step_end):
            is_first_field = index == step_start
            if not is_first_field and indentation(lines[index]) != 8:
                continue
            field = yaml_field(lines[index])
            if field is None:
                continue
            direct_fields.append(field)
            if field == ("run", EXPECTED_CI_BUILD):
                direct_build_run_indexes.append(index)
        if ("run", EXPECTED_CI_BUILD) in direct_fields:
            direct_build_runs.append((step_start, direct_fields))

    all_build_run_indexes = [
        index
        for index in range(job_start + 1, job_end)
        if yaml_field(lines[index]) == ("run", EXPECTED_CI_BUILD)
    ]
    if len(direct_build_runs) != 1 or len(direct_build_run_indexes) != 1:
        errors.append(
            "the e2e-tests job must associate exactly one direct step run with "
            f"`{EXPECTED_CI_BUILD}`"
        )
    else:
        _, build_fields = direct_build_runs[0]
        run_values = [value for key, value in build_fields if key == "run"]
        if run_values != [EXPECTED_CI_BUILD]:
            errors.append(
                "the E2E build step must define exactly one direct run field equal to "
                f"`{EXPECTED_CI_BUILD}`"
            )
        if any(key == "if" for key, _ in build_fields):
            errors.append("the E2E build step must not define `if:` and risk being disabled")
        if any(key == "continue-on-error" for key, _ in build_fields):
            errors.append("the E2E build step must not define `continue-on-error:`")

    if all_build_run_indexes != direct_build_run_indexes:
        errors.append(
            "the E2E build command must not appear outside its enabled build step"
        )

    e2e_job = "\n".join(lines[job_start + 1 : job_end])
    if "npm run build" in e2e_job:
        errors.append(
            "the e2e-tests job must not duplicate the frontend `npm run build` sequence"
        )
    if "cargo build" in e2e_job:
        errors.append(
            "the e2e-tests job must not duplicate backend `cargo build` commands"
        )
    return errors


VALID_MAKEFILE = '''NEXT_PUBLIC_API_URL ?= http://localhost:4001

e2e-build: test-ci-config
\tcd backend && cargo build --release --bin cronometrix
\tcd backend && cargo build --release --bin mock_hikvision --features mock-hikvision
\tcd backend && cargo build --release --bin seed_e2e --features seed-e2e
\tcd frontend && NEXT_PUBLIC_API_URL="$(NEXT_PUBLIC_API_URL)" npm run build

e2e: e2e-build
\t@true
'''

VALID_PLAYWRIGHT = '''export default defineConfig({
  webServer: [
    {
      command: isE2ERelease
        ? "../backend/target/release/cronometrix"
        : "../backend/target/debug/cronometrix",
      url: "http://127.0.0.1:4001/api/v1/health",
      env: {
        SERVER_PORT: "4001",
        CORS_ALLOWED_ORIGINS: "http://localhost:3001",
        COOKIE_SECURE: "false",
      },
    },
    {
      command: "next start --port 3001",
    },
  ],
});
'''

VALID_WORKFLOW = '''jobs:
  other-job:
    steps:
      - run: echo other
  e2e-tests:
    name: E2E Tests
    runs-on: ubuntu-latest
    steps:
      - name: Build E2E release harness
        run: make -C "$GITHUB_WORKSPACE" e2e-build
      - name: Upload report
        if: always()
        run: echo upload
  later-job:
    steps:
      - run: echo later
'''


def run_self_tests() -> int:
    failures: list[str] = []
    count = 0

    def expect_valid(name: str, validator, fixture: str) -> None:
        nonlocal count
        count += 1
        found = validator(fixture)
        if found:
            failures.append(f"{name}: valid fixture rejected: {'; '.join(found)}")

    def expect_invalid(
        name: str, validator, fixture: str, expected_error: str
    ) -> None:
        nonlocal count
        count += 1
        found = validator(fixture)
        if not any(expected_error in error for error in found):
            rendered = "; ".join(found) if found else "no errors"
            failures.append(
                f"{name}: expected `{expected_error}`, observed: {rendered}"
            )

    expect_valid("make-valid", validate_makefile, VALID_MAKEFILE)
    expect_invalid(
        "make-duplicate-rule",
        validate_makefile,
        VALID_MAKEFILE + "\ne2e-build:\n\t@true\n",
        "exactly one e2e-build rule",
    )
    expect_invalid(
        "make-duplicate-npm-build",
        validate_makefile,
        VALID_MAKEFILE.replace(
            EXPECTED_BUILD_LINE,
            EXPECTED_BUILD_LINE
            + "\n\n# recipe comment\n\tcd frontend && npm run build",
            1,
        ),
        "exactly one effective `npm run build`",
    )
    expect_invalid(
        "make-multi-target-duplicate-rule",
        validate_makefile,
        VALID_MAKEFILE + "\nhelper e2e-build:\n\t@true\n",
        "exactly one e2e-build rule",
    )
    expect_invalid(
        "make-extra-effective-command",
        validate_makefile,
        VALID_MAKEFILE.replace(
            EXPECTED_BUILD_LINE,
            EXPECTED_BUILD_LINE + "\n\t$(EXTRA_BUILD)",
            1,
        ),
        "entire effective e2e-build recipe",
    )

    expect_valid("playwright-valid", validate_playwright, VALID_PLAYWRIGHT)
    backend_identity = (
        '      command: isE2ERelease\n'
        '        ? "../backend/target/release/cronometrix"\n'
        '        : "../backend/target/debug/cronometrix",\n'
        '      url: "http://127.0.0.1:4001/api/v1/health",\n'
    )
    expect_invalid(
        "playwright-next-first-server",
        validate_playwright,
        VALID_PLAYWRIGHT.replace(
            backend_identity,
            '      command: "next start --port 3001",\n'
            '      url: "http://localhost:3001/login",\n',
            1,
        ),
        "exact release/debug cronometrix command",
    )
    expect_invalid(
        "playwright-late-env-spread",
        validate_playwright,
        VALID_PLAYWRIGHT.replace(
            '        COOKIE_SECURE: "false",\n',
            '        COOKIE_SECURE: "false",\n        ...process.env,\n',
            1,
        ),
        "spread after CORS/cookie assignments",
    )
    env_assignments = "\n".join(EXPECTED_BACKEND_ENV.values()) + "\n"
    playwright_outside_only = VALID_PLAYWRIGHT.replace(env_assignments, "", 1)
    playwright_outside_only = playwright_outside_only.replace(
        "      },\n    },",
        "      },\n      metadata: {\n" + env_assignments + "      },\n    },",
        1,
    )
    expect_invalid(
        "playwright-values-outside-env",
        validate_playwright,
        playwright_outside_only,
        "env must set exactly",
    )
    playwright_duplicate_outside = VALID_PLAYWRIGHT.replace(
        "      },\n    },",
        "      },\n      metadata: {\n" + env_assignments + "      },\n    },",
        1,
    )
    expect_invalid(
        "playwright-duplicate-outside-env",
        validate_playwright,
        playwright_duplicate_outside,
        "outside env",
    )

    expect_valid("workflow-valid", validate_workflow, VALID_WORKFLOW)
    build_step = (
        "      - name: Build E2E release harness\n"
        f"        run: {EXPECTED_CI_BUILD}\n"
    )
    expect_invalid(
        "workflow-duplicate-direct-run",
        validate_workflow,
        VALID_WORKFLOW.replace(
            build_step,
            build_step + "        run: echo bypass\n",
            1,
        ),
        "exactly one direct run field",
    )
    expect_invalid(
        "workflow-continue-on-error",
        validate_workflow,
        VALID_WORKFLOW.replace(
            build_step,
            build_step + "        continue-on-error: true\n",
            1,
        ),
        "must not define `continue-on-error:`",
    )
    expect_invalid(
        "workflow-command-outside-step",
        validate_workflow,
        VALID_WORKFLOW.replace(
            build_step,
            "      - name: Build E2E release harness\n"
            "        env:\n"
            f"          run: {EXPECTED_CI_BUILD}\n",
            1,
        ),
        "direct step run",
    )
    expect_invalid(
        "workflow-disabled-build-step",
        validate_workflow,
        VALID_WORKFLOW.replace(
            build_step,
            "      - name: Build E2E release harness\n"
            "        if: ${{ false }}\n"
            f"        run: {EXPECTED_CI_BUILD}\n",
            1,
        ),
        "build step must not define `if:`",
    )
    expect_invalid(
        "workflow-disabled-job",
        validate_workflow,
        VALID_WORKFLOW.replace(
            "  e2e-tests:\n", "  e2e-tests:\n    if: ${{ false }}\n", 1
        ),
        "job must not define `if:`",
    )
    expect_invalid(
        "workflow-command-in-other-job",
        validate_workflow,
        VALID_WORKFLOW.replace(build_step, "", 1).replace(
            "      - run: echo other",
            f"      - run: {EXPECTED_CI_BUILD}",
            1,
        ),
        "direct step run",
    )
    expect_invalid(
        "workflow-legacy-command",
        validate_workflow,
        VALID_WORKFLOW.replace(EXPECTED_CI_BUILD, "make e2e-build", 1),
        "direct step run",
    )
    expect_invalid(
        "workflow-duplicate-command-outside-step",
        validate_workflow,
        VALID_WORKFLOW.replace(
            "    runs-on: ubuntu-latest\n"
            "    steps:\n",
            "    runs-on: ubuntu-latest\n"
            "    env:\n"
            f"      run: {EXPECTED_CI_BUILD}\n"
            "    steps:\n",
            1,
        ),
        "outside its enabled build step",
    )

    if failures:
        print(
            "\n".join(f"FAIL: E2E guard self-test {failure}" for failure in failures),
            file=sys.stderr,
        )
        raise SystemExit(1)
    return count


self_test_count = run_self_tests()
print(f"PASS: E2E harness guard self-tests ({self_test_count} cases)")

repo_root = Path(sys.argv[1])
paths = {
    "Makefile": repo_root / "Makefile",
    "frontend/playwright.config.ts": repo_root / "frontend/playwright.config.ts",
    ".github/workflows/ci.yml": repo_root / ".github/workflows/ci.yml",
}
missing = [relative for relative, path in paths.items() if not path.is_file()]
if missing:
    print(
        "\n".join(
            f"FAIL: missing E2E harness configuration file: {relative}"
            for relative in missing
        ),
        file=sys.stderr,
    )
    raise SystemExit(1)

errors = [
    *validate_makefile(paths["Makefile"].read_text()),
    *validate_playwright(paths["frontend/playwright.config.ts"].read_text()),
    *validate_workflow(paths[".github/workflows/ci.yml"].read_text()),
]
if errors:
    print("\n".join(f"FAIL: {error}" for error in errors), file=sys.stderr)
    raise SystemExit(1)

print("PASS: E2E harness configuration contracts")
PY
