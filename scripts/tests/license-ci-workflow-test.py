#!/usr/bin/env python3
from pathlib import Path

import yaml


repo_root = Path(__file__).resolve().parents[2]
workflow = yaml.safe_load((repo_root / ".github/workflows/ci.yml").read_text())
job = workflow["jobs"]["license-functions"]

assert job["name"] == "License Functions"
assert job["permissions"] == {"contents": "read"}

steps = job["steps"]
setup_node = next(step for step in steps if step.get("uses") == "actions/setup-node@v4")
assert setup_node["with"]["node-version-file"] == ".nvmrc"
assert setup_node["with"]["cache"] == "npm"
assert setup_node["with"]["cache-dependency-path"] == "do-functions/package-lock.json"

commands = "\n".join(str(step.get("run", "")) for step in steps)
for required in (
    "npm ci",
    "npm test",
    "bash scripts/tests/license-secret-tools-test.sh",
    "bash scripts/tests/deploy-license-authority-test.sh",
):
    assert required in commands

node_test_step = next(step for step in steps if "npm ci" in str(step.get("run", "")))
assert node_test_step["working-directory"] == "do-functions"

print("PASS: license Functions CI contract")
