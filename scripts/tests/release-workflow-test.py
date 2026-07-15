#!/usr/bin/env python3
from pathlib import Path

import yaml


workflow_path = Path(__file__).resolve().parents[2] / ".github/workflows/release.yml"
workflow = yaml.safe_load(workflow_path.read_text())

assert workflow["name"] == "Release"
assert workflow["on"]["push"]["branches"] == ["codex/release-build-*"]
assert workflow["on"]["push"]["tags"] == ["v*"]
assert "workflow_dispatch" not in workflow["on"]
assert workflow["permissions"] == {"contents": "read"}
assert workflow["concurrency"] == {
    "group": "release-${{ github.ref }}",
    "cancel-in-progress": False,
}

jobs = workflow["jobs"]
assert jobs["build-images"]["permissions"] == {
    "contents": "read",
    "packages": "write",
}
assert jobs["promote-images"]["permissions"] == {
    "contents": "read",
    "checks": "read",
    "packages": "write",
}
assert jobs["build-images"]["if"] == (
    "github.event_name == 'push' && "
    "startsWith(github.ref, 'refs/heads/codex/release-build-')"
)
assert jobs["promote-images"]["if"] == (
    "github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')"
)
assert jobs["promote-images"]["environment"] == "release"
assert jobs["bundle"]["needs"] == ["build-images", "promote-images"]

bundle_if = jobs["bundle"]["if"]
for fragment in (
    "always()",
    "needs.build-images.result == 'success'",
    "needs.promote-images.result == 'success'",
    "needs.build-images.result == 'skipped'",
    "needs.promote-images.result == 'skipped'",
):
    assert fragment in bundle_if

build_text = str(jobs["build-images"])
for required in (
    "git rev-parse",
    "HEAD^{commit}",
    "GITHUB_SHA",
    "GITHUB_REF_NAME",
    "codex/release-build-${SOURCE_SHA}",
    "linux/amd64",
    "NEXT_PUBLIC_API_URL=",
    "org.opencontainers.image.revision",
):
    assert required in build_text

promote_text = str(jobs["promote-images"])
for required in (
    "Backend Coverage",
    "Frontend Coverage",
    "E2E Tests",
    "Container Smoke",
    "Release Gate",
    "imagetools create",
    "org.opencontainers.image.revision",
):
    assert required in promote_text

workflow_text = workflow_path.read_text()
for action in (
    "docker/login-action@v3",
    "docker/setup-buildx-action@v3",
    "docker/build-push-action@v6",
    "actions/upload-artifact@v4",
):
    assert action in workflow_text

for name, job in jobs.items():
    if name not in {"build-images", "promote-images"}:
        assert job.get("permissions", workflow["permissions"]).get("packages") != "write"

bundle_text = str(jobs["bundle"])
for member in (
    "install.sh",
    "docker-compose.yml",
    "release-manifest.env",
    "nginx.conf",
    "SHA256SUMS",
    "retention-days",
    "14",
):
    assert member in bundle_text

print("PASS: release workflow contract")
