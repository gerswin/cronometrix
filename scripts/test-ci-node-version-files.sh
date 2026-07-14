#!/usr/bin/env bash
set -euo pipefail

repo_root="${CRONOMETRIX_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
workflow_rel=".github/workflows/ci.yml"
workflow="$repo_root/$workflow_rel"

python3 - "$workflow" "$repo_root" "$workflow_rel" <<'PY'
from pathlib import Path
import re
import sys

workflow = Path(sys.argv[1])
repo_root = Path(sys.argv[2]).resolve()
workflow_rel = sys.argv[3]

if not workflow.is_file():
    raise SystemExit(f"FAIL: missing workflow: {workflow_rel}")

lines = workflow.read_text().splitlines()


def indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def step_blocks() -> list[list[str]]:
    blocks: list[list[str]] = []
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.strip() != "steps:":
            index += 1
            continue
        steps_indent = indent(line)
        index += 1
        item_indent: int | None = None
        current: list[str] = []
        while index < len(lines):
            candidate = lines[index]
            stripped = candidate.strip()
            candidate_indent = indent(candidate)
            if stripped and not stripped.startswith("#") and candidate_indent <= steps_indent:
                break
            if stripped.startswith("- "):
                if item_indent is None:
                    item_indent = candidate_indent
                if candidate_indent == item_indent:
                    if current:
                        blocks.append(current)
                    current = [candidate]
                    index += 1
                    continue
            if current:
                current.append(candidate)
            index += 1
        if current:
            blocks.append(current)
    return blocks


def normalized_key(line: str) -> str:
    value = line.strip()
    return value[2:].lstrip() if value.startswith("- ") else value


def uses_setup_node(block: list[str]) -> bool:
    return any(
        normalized_key(line).startswith("uses:")
        and normalized_key(line).split(":", 1)[1].strip().strip("\"'").startswith("actions/setup-node@")
        for line in block
    )


def version_files(block: list[str]) -> list[str]:
    results: list[str] = []
    for index, line in enumerate(block):
        if normalized_key(line) != "with:":
            continue
        with_indent = indent(line)
        for nested in block[index + 1 :]:
            stripped = nested.strip()
            if stripped and indent(nested) <= with_indent:
                break
            key = normalized_key(nested)
            if key.startswith("node-version-file:"):
                raw = key.split(":", 1)[1].strip()
                if raw.startswith(("\"", "'")) and raw.endswith(raw[0]):
                    raw = raw[1:-1]
                results.append(raw)
    return results


setup_steps = [block for block in step_blocks() if uses_setup_node(block)]
if not setup_steps:
    raise SystemExit(f"FAIL: {workflow_rel} has no setup-node steps")

errors: list[str] = []
for number, block in enumerate(setup_steps, 1):
    paths = version_files(block)
    if len(paths) != 1:
        errors.append(
            f"FAIL: {workflow_rel} setup-node step {number} has {len(paths)} node-version-file entries"
        )
        continue
    referenced_path = paths[0]
    if not referenced_path or "${{" in referenced_path:
        errors.append(
            f"FAIL: {workflow_rel} setup-node step {number} has a non-literal node-version-file"
        )
        continue
    candidate = (repo_root / referenced_path).resolve()
    try:
        candidate.relative_to(repo_root)
    except ValueError:
        errors.append(
            f"FAIL: {workflow_rel} setup-node step {number} escapes the repository: {referenced_path}"
        )
        continue
    if not candidate.is_file():
        errors.append(
            f"FAIL: {workflow_rel} references missing node-version-file: {referenced_path}"
        )

if errors:
    print("\n".join(errors), file=sys.stderr)
    raise SystemExit(1)
PY
