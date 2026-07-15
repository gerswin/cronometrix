# Phase 12 Plan 12-04 — Private distribution and same-origin gateway

Verdict: PASS — scoped distribution checkpoint

Release gate: FAIL — deferred to 12-05

The same-origin gateway, private digest-pinned image workflow, checksum-verified
installer bundle, transactional installer contract, local container topology,
and private dry release all passed. This is not an unqualified v1.0 release
verdict. Plan 12-05 still owns the immutable final candidate, the fifth
aggregate `Release Gate` check, and convergence of every release check on that
candidate SHA. No v1.0 tag was created.

## Identity

- Branch: `codex/phase12-04-private-distribution`
- Plan base SHA: `6453a4ceb8d3d2f23ccf1fb9d6f3c291cc81dec1`
- Dry-release source SHA: `683c5af290b62e52232851b5fb380988249093be`
- Coverage ownership commit: `683c5af` (`test(deploy): freeze plan-owned coverage scope`)
- One-use release ref:
  `codex/release-build-683c5af290b62e52232851b5fb380988249093be`
- Release run: [29386428370](https://github.com/gerswin/cronometrix/actions/runs/29386428370)
- Same-SHA CI run: [29386428374](https://github.com/gerswin/cronometrix/actions/runs/29386428374)

The original `/tmp/cronometrix-12-04-base-sha` marker was no longer present
when Task 5 resumed. It was restored to the immutable branch-creation SHA from
the reflog (`branch: Created from origin/main`) and independently confirmed as
the sole parent of the first 12-04 implementation commit. It was not derived
from the later ownership manifest or current HEAD.

The two follow-up 12-01 lint commits were merged before freezing the dry
release SHA:

- `a2634cf` — strict backend Clippy baseline;
- `587463a` — frontend lint cleanup.

## Delivered distribution contracts

### Same-origin runtime

- Browser API and SSE traffic use a path-only public base when
  `NEXT_PUBLIC_API_URL` is explicitly empty.
- Server-side proxy code uses `INTERNAL_API_URL=http://api:3001` without
  leaking Docker DNS into browser HTML or JavaScript.
- Nginx is the only browser-facing origin. It routes `/api/*` to Axum and all
  other paths to Next.js; Cloudflare Tunnel targets `gateway:8080` only.
- The exact SSE location disables access logging, suppresses ordinary
  request-bearing error logs, disables buffering/cache, and retains the long
  read timeout.
- API and web have no host ports in production Compose. Gateway binds only
  `127.0.0.1:8080` for local operator access.
- The web image runs Next.js standalone output instead of the incompatible
  `next start` path recorded by 12-01/12-02.

### Private immutable release workflow

- Candidate branch pushes build private `linux/amd64` API, web, and gateway
  images in GHCR and record their OCI digests.
- Production base images and cloudflared are digest-pinned.
- The production API image contains only `cronometrix`; test seed binaries,
  Python, demo auto-seeding, and the former demo entrypoint are absent.
- Tag promotion is a separate protected-environment path which aliases the
  already-tested SHA digests without rebuilding. It was intentionally skipped
  for this branch dry run.
- The private Actions artifact contains exactly `install.sh`,
  `docker-compose.yml`, `release-manifest.env`, `nginx.conf`, and
  `SHA256SUMS`, plus the external tarball checksum alongside the archive.

### Transactional installation

- The installer rejects anonymous `curl | bash`, mutable image references,
  malformed manifests, unsafe archives, unsupported platforms, and secrets as
  positional arguments.
- GHCR authentication uses `--password-stdin` and a root-only dedicated
  Docker config. The token is not copied to runtime `.env`.
- Bundle checksums and the release manifest are validated before login or
  installation state changes.
- Existing secrets and data are preserved; Compose is validated before pull;
  current manifests and SQLite/WAL state are backed up; failed health checks
  restore and restart the prior release.
- Local API/setup/gateway/upload/container health must pass before
  cloudflared starts. Only the two newest successful rollback directories are
  retained.

## Dry-release evidence

The Release workflow completed successfully on the exact source SHA. All
three image jobs and the bundle job passed; the promotion job was correctly
skipped for a branch build.

| Artifact | Immutable identity |
|---|---|
| Release version | `sha-683c5af290b6` |
| API | `ghcr.io/gerswin/cronometrix-api:sha-683c5af290b6@sha256:67c4386b79be0e39c07f1d9f7885d89d3186499e255cf3fb4c3fc2f0a03408c1` |
| Web | `ghcr.io/gerswin/cronometrix-web:sha-683c5af290b6@sha256:91b312cb360d328ce0a2e28902d136560e6ec595ce4a0ac08907d697b97fb429` |
| Gateway | `ghcr.io/gerswin/cronometrix-gateway:sha-683c5af290b6@sha256:133539b67493ef87e02d90b7be344c404db6f595a62deae653354b825751a8fe` |
| Cloudflared | `cloudflare/cloudflared:2026.3.0@sha256:6b599ca3e974349ead3286d178da61d291961182ec3fe9c505e1dd02c8ac31b0` |
| Bundle SHA-256 | `5c2c085f69edc53a7693fee5ccd60cf6034584ac97c38e56324b8b6f0cb9704e` |

Authenticated artifact verification proved:

- the external `.tar.gz.sha256` matches the archive;
- the member list is exactly the five allowed files;
- every internal `SHA256SUMS` entry matches;
- the manifest passes the strict allowlist verifier;
- `SOURCE_SHA=683c5af290b62e52232851b5fb380988249093be`;
- downloaded archives and extracted files were removed after validation.

## Coverage and verification ledger

### Frontend

| Verification | Result |
|---|---|
| Focused API-base/API/SSE tests | PASS — 36/36 |
| Production Next.js build | PASS |
| Full Vitest coverage | PASS — 472/472 |
| Statements | 1498/1625 = 92.18% (floor 90%) |
| Branches | 936/1089 = 85.95% (floor 85%) |
| Functions | 442/475 = 93.05% (floor 90%) |
| Lines | 1388/1476 = 94.03% (floor 90%) |

### Backend

| Platform | Tests | Lines | Branches | Target exit |
|---|---:|---:|---:|---:|
| macOS arm64 | 989 passed, 22 skipped | 11471/12886 = 89.02% | 911/1102 = 82.67% | 2 |
| Linux/arm64 Docker | 1000 passed, 22 skipped | 11629/12889 = 90.22% | 939/1102 = 85.21% | 0 |

The macOS result is the documented platform limitation for
`license/fingerprint.rs` and `license/service.rs`: Darwin lacks the Linux
`/proc/cpuinfo` and `/sys/{class/net,block}` sources. No threshold or exclusion
was changed. Linux is the deployment-target and CI-authoritative result; it
passed the 90% line gate, the 85% project branch gate, and the 70% line / 60%
branch per-file floors.

The first Linux container attempt was terminated by the linker under parallel
memory pressure. Re-running the identical coverage command with
`CARGO_BUILD_JOBS=1` completed successfully. LCOV's generated `/workspace/`
source prefix was normalized to the real host checkout path before the
containment-aware ownership checker ran.

Exact owned checker result:

```text
PASS owned-coverage plan=12-04 backend=5 frontend=3
```

The ownership manifest includes every changed covered production file from
the frozen plan base, including five backend files delivered by the coverage
remediation merged after branch creation and three frontend files.

### Functional and distribution checks

| Verification | Result |
|---|---|
| Strict release-manifest fixture matrix | PASS |
| Static transactional-installer contract | PASS |
| Same-origin container smoke | PASS |
| Gateway/API/login routing and upload limits | PASS |
| Production API test-reset route disabled | PASS — 404 |
| Gateway/API restart and retained data | PASS |
| SSE success, unauthorized, and upstream-failure marker leak checks | PASS — zero complete markers in repository-controlled logs |
| DB lock/secret log scan and cleanup | PASS |
| Release workflow image jobs and bundle | PASS |
| External and internal bundle checksums | PASS |
| Same-SHA Backend Coverage | PASS |
| Same-SHA Frontend Coverage | PASS |
| Same-SHA E2E Tests | PASS |
| Same-SHA Container Smoke | PASS |

## Residual risks and 12-05 handoff

- The CI run has four green checks on the dry-release SHA, but the aggregate
  `Release Gate` check does not exist until 12-05. This scoped checkpoint must
  not be promoted as the final candidate.
- The one-use ref is immutable evidence for this mechanism only. Plan 12-05
  creates a new final candidate after its Tasks 1–4 and repeats both CI and
  the private release workflow on that exact SHA.
- GitHub emitted deprecation annotations because pinned third-party Actions
  still declare Node 20; GitHub forced them onto Node 24 and every affected
  job passed. Track upstream action upgrades without weakening action pinning.
- `npm ci` still reports 16 dependency advisories (1 low, 7 moderate, 8 high).
  No automatic dependency mutation was authorized; security triage remains
  release debt.
- Repository-controlled gateway/application/container logs proved that full
  SSE query tokens are absent for success, unauthorized, and upstream-failure
  paths. Browser diagnostics/history and Cloudflare account-side telemetry
  remain outside this harness. Mitigations remain short JWT lifetime, no URL
  sharing, and restricted Cloudflare log access/retention.
- Real Hikvision hardware/firmware/network validation and cross-host LIC-05
  anti-cloning validation remain explicit live-environment work. Mock and unit
  evidence do not replace those proofs.
- The production installer was proven statically and the container topology
  dynamically. Fresh-VM install, restart/offline, idempotence, rollback, and
  cleanup evidence remains Phase 13 work.

Handoff to 12-05:

- dry-release SHA: `683c5af290b62e52232851b5fb380988249093be`;
- release run: [29386428370](https://github.com/gerswin/cronometrix/actions/runs/29386428370);
- same-SHA CI run: [29386428374](https://github.com/gerswin/cronometrix/actions/runs/29386428374);
- bundle and image identities: the table above;
- no v1.0 tag or release promotion has occurred.
