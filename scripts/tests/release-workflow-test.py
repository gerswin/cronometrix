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
arm_job = jobs["build-api-arm64-binary"]
assert arm_job["runs-on"] == [
    "self-hosted",
    "Linux",
    "ARM64",
    "caladan",
    "cronometrix-release",
]
assert arm_job["permissions"] == {"contents": "read"}
assert arm_job["if"] == jobs["build-images"]["if"]
assert jobs["build-images"]["needs"] == ["build-api-arm64-binary"]
arm_text = str(arm_job)
for required in ("api-binary-export", "linux/arm64", "api-arm64-binary"):
    assert required in arm_text

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
    "NEXT_PUBLIC_API_URL=",
    "org.opencontainers.image.revision",
):
    assert required in build_text

build_push_step = next(
    step
    for step in jobs["build-images"]["steps"]
    if step.get("uses") == "docker/build-push-action@v6"
)
assert build_push_step["with"]["platforms"] == "linux/amd64,linux/arm64"
assert build_push_step["with"]["cache-from"] == "type=gha,scope=release-${{ matrix.component }}"
assert build_push_step["with"]["cache-to"] == (
    "type=gha,mode=max,scope=release-${{ matrix.component }}"
)
assert "api-arm64-binary" in build_text

build_actions = [step.get("uses") for step in jobs["build-images"]["steps"]]
qemu_index = build_actions.index("docker/setup-qemu-action@v3")
buildx_index = build_actions.index("docker/setup-buildx-action@v3")
build_push_index = build_actions.index("docker/build-push-action@v6")
assert qemu_index < buildx_index < build_push_index

promote_text = str(jobs["promote-images"])
for required in (
    "Backend Coverage",
    "Frontend Coverage",
    "E2E Tests",
    "Container Smoke",
    "License Functions",
    "Release Gate",
    "imagetools create",
    "org.opencontainers.image.revision",
):
    assert required in promote_text

workflow_text = workflow_path.read_text()
for action in (
    "docker/login-action@v3",
    "docker/setup-qemu-action@v3",
    "docker/setup-buildx-action@v3",
    "docker/build-push-action@v6",
    "actions/upload-artifact@v4",
):
    assert action in workflow_text

for name, job in jobs.items():
    if name not in {"build-images", "promote-images"}:
        assert job.get("permissions", workflow["permissions"]).get("packages") != "write"

bundle_text = str(jobs["bundle"])
for required in (
    "scripts/assemble-release-bundle.sh",
    "retention-days",
    "14",
):
    assert required in bundle_text

dockerfile = (workflow_path.parents[2] / "deploy/Dockerfile.api").read_text()
for required in (
    "AS api-binary-export",
    "AS binary-amd64",
    "AS binary-arm64",
    "COPY prebuilt/cronometrix-arm64",
    "FROM binary-${TARGETARCH} AS selected-binary",
    "COPY --from=selected-binary",
):
    assert required in dockerfile

print("PASS: release workflow contract")
