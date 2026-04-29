# Phase 11: v1.0 Live Environment Validation — Context

**Gathered:** 2026-04-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Execute every audit-deferred validation item that requires real (non-CI-mockable) infrastructure and capture committed evidence so milestone v1.0 can be archived with a clean human-verified verdict. Scope:

1. Phase 8 + 9 live CI green run (push branch, confirm `Backend Coverage` + `Frontend Coverage` + `E2E Tests` all pass on real GitHub Actions runner)
2. Phase 8 + 9 live CI red regression run (open deliberate broken PR, confirm hard-fail behavior across all 3 gates)
3. Branch protection toggle (require all 3 status checks before merge to `main`)
4. Local `make e2e` end-to-end run against real dev stack (72 Playwright specs)
5. Phase 6 fresh-VM installer smoke (Ubuntu 22.04 + Docker + real Cloudflare token + DO Functions URL)

Mostly evidence-gathering and process work — minimal new code, no new features. LIC-05 cross-host clone-rejection test is **explicitly deferred to first production install** (D-09). Real Hikvision alertStream test is **out of Phase 11 scope** (D-10 — covered by mock_hikvision in Phase 9).

</domain>

<decisions>
## Implementation Decisions

### Evidence storage & format
- **D-01:** Fully committed evidence directory: every item produces evidence inside `.planning/phases/11-v1-0-live-environment-validation/evidence/{NN-item-name}/`. Markdown report (`README.md` per item) + binary artifacts (PNG screenshots, downloaded Playwright HTML reports, raw terminal log captures `.txt`) all committed to git.
- **D-02:** Evidence README format per item:
  ```markdown
  # Evidence: {item name}
  - **Date captured:** YYYY-MM-DD
  - **Captured by:** {operator name / username}
  - **Command run / action:** `{exact command or UI step}`
  - **Expected:** {what should happen}
  - **Actual:** {what did happen}
  - **Verdict:** PASS | FAIL
  - **Artifacts:** [list of files in this dir]
  - **External refs (if any):** {GH Actions run URL, PR number}
  ```
- **D-03:** Repo bloat acknowledged. Playwright HTML reports can be 5–20 MB each; total Phase 11 evidence dir size budget = **150 MB** (decision-record cost we accept for offline-auditable proof). If a single item's HTML report exceeds 50 MB, compress with `tar.gz` before committing rather than referencing externally.
- **D-04:** No `.gitignore` exclusion for `evidence/` — the whole directory is intentionally tracked. Add to `.gitattributes` with `*.html linguist-generated=true` so GitHub web UI doesn't pollute language stats with HTML report blobs.
- **D-05:** Evidence README links to artifact files using relative paths (e.g., `[Playwright HTML report](./playwright-report.tar.gz)`) so the docs work both on disk and on GitHub web UI.

### LIC-05 cross-host clone-rejection test
- **D-06:** Defer LIC-05 cross-host clone test to first production install. Phase 11 evidence for LIC-05 captures: (a) the abort-mismatch code path is exercised by existing unit/integration tests in `backend/tests/license_*` (in-process synthetic mismatch), and (b) a documented risk-acceptance note that real-hardware verification will happen at first paying-customer deploy.
- **D-07:** Phase 11 evidence directory MUST include `evidence/05-lic-05-deferral/README.md` capturing the deferral rationale + risk acknowledgement (highest risk of all deferrals: a license-binding failure at customer deploy is highly visible). Include the exact pre-flight checklist a deploy engineer should run on first install: spawn install on machine A → activate → snapshot fingerprint → relocate license JWT to machine B → confirm activation rejected.
- **D-08:** Update `06-VERIFICATION.md` deferred-items table to cross-reference this Phase 11 evidence file as the authoritative deferral record (keeps audit traceability).
- **D-09:** Add `LIC-05-CROSS-HOST` to the `## v1.1 Backlog` section in REQUIREMENTS.md (alongside DEPL-03-AUTO from Phase 10's traceability refresh) with the entry: "Real-hardware cross-host clone test on 2 distinct machines or 2 cloud VMs. Currently deferred to first production deployment evidence."

### Real Hikvision live test
- **D-10:** Out of Phase 11 scope. The integration is sufficiently exercised by `mock_hikvision.rs` (Phase 9) for digest-auth + alertStream protocol. Real-hardware test happens at first install where real device is connected — same risk-accept pattern as D-06. Note in `evidence/06-hikvision-deferral/README.md` (or fold into D-07's risk doc).

### Sequencing & completion criteria
- **D-11:** Block-until-complete execution. Phase 11 does NOT start until prerequisites are confirmed in one place:
  - GitHub Actions runner billing budget available for ~3-5 live runs
  - 1 fresh Ubuntu 22.04 VM provisioned (cloud or local) with Docker + ~10 GB disk + outbound internet
  - 1 valid Cloudflare API tunnel TOKEN (operator pre-creates tunnel in CF Zero Trust dashboard)
  - 1 valid DO Functions URL for license activation
  - Local dev environment ready for `make e2e` (Rust nightly + Node 20 + Playwright chromium browser cached)
- **D-12:** Estimated session duration: 2-4 hours of focused work once prerequisites are ready.
- **D-13:** Phase 11 verification gate: ALL 5 evidence items must show `Verdict: PASS` (or in LIC-05's case, `Verdict: DEFERRED` with risk-accept doc) before STATE.md flips to `complete` and milestone-complete is unblocked.
- **D-14:** Resumability: although the phase is "block-until-complete" by intent, evidence items MUST commit as they finish (one item = one atomic commit per Phase 9's pattern). If the session is interrupted, the next session resumes from the first incomplete item.

### Cross-cutting: change minimization
- **D-15:** Phase 11 SHOULD NOT modify `backend/`, `frontend/`, or `.github/workflows/` source code unless an evidence run reveals a regression. The expected production code change is **zero**. The expected planning/docs change is small (the evidence dir, plus a STATE/ROADMAP closeout commit).
- **D-16:** Branch protection toggle is a GitHub UI action — not a file edit. Capture screenshot of `Settings → Branches → main → Require status checks` showing the 3 required checks enabled. Save as `evidence/03-branch-protection/screenshot.png`.
- **D-17:** If the live red regression PR reveals an unexpected gate failure (false negative — gate doesn't fail when it should), STOP and treat as P0 bug → reroute through `/gsd-debug`. Phase 11's job is verification, not bug-fixing.

### Claude's Discretion
- Wave-grouping for execution: planner decides how to chunk the 5 items. Reasonable default is 3 waves: Wave 1 = items 1+2+4 (CI green, CI red, local make e2e — all software-only); Wave 2 = item 3 (branch protection — depends on Wave 1 confirming jobs exist); Wave 3 = item 5 (fresh-VM installer — independent track, can actually run in parallel with Wave 1 if VM is provisioned ready).
- Cloud-VM provider for the fresh-VM smoke: DigitalOcean / Hetzner / Linode / local KVM — operator picks based on available accounts + cost preference. Reproducibility script (`evidence/05-installer-smoke/provision.sh` or equivalent) is nice-to-have but not required.
- Evidence README naming: `00-overview.md` for the index page is recommended but planner can name it differently.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Audit context
- `.planning/v1.0-MILESTONE-AUDIT.md` — full audit; Phase 11 closes the live-env tech_debt items
- `.planning/phases/10-v1-0-documentation-and-sign-off-hardening/10-CONTEXT.md` — Phase 10 sister phase; ensures Phase 11 doesn't duplicate Phase 10's traceability work

### Phase 8 manual follow-up source
- `CLAUDE.md` §Test Coverage § Pending live validation — exact 3-step checklist (positive PR / negative PR / branch protection)
- `.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-05-SUMMARY.md` § Manual Follow-up
- `.github/workflows/ci.yml` — `Backend Coverage` + `Frontend Coverage` job definitions (must remain byte-identical post-Phase 11)

### Phase 9 manual follow-up source
- `CLAUDE.md` §End-to-End Tests § Pending live validation — checklist mirroring Phase 8 pattern
- `.planning/phases/09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea/09-12-SUMMARY.md` § Manual Follow-up
- `.planning/phases/09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea/09-13-SUMMARY.md` § Phase 9 Close-out checklist
- `.planning/phases/09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea/09-VERIFICATION.md` § Human Verification Required (4 items)
- `.github/workflows/ci.yml` — `E2E Tests` job (job name MUST match exactly for branch protection)

### Phase 6 fresh-VM installer source
- `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` § human_verification — fresh-VM smoke contract
- `.planning/phases/06-licensing-deployment/06-CONTEXT.md` D-13 — token-based connector design choice
- `deploy/install.sh` — installer script under test
- `docker-compose.yml` — 3-service deploy (api + web + cloudflared)
- `.env.example` (or equivalent) — required env vars: CRONOMETRIX_*, CLOUDFLARE_TUNNEL_TOKEN, DO Functions URL, CLIENT_SLUG

### LIC-05 deferral evidence
- `backend/tests/license_*` — existing in-process license tests (sufficient for code-path coverage)
- `backend/src/license/fingerprint.rs` — fingerprint formula (CPU + MAC + disk serial)
- `backend/src/license/service.rs` — license validation + bypass safety
- `.planning/phases/06-licensing-deployment/06-04-SUMMARY.md` — license-binding implementation

### Repository hygiene
- `.gitignore` — verify no exclusions block `evidence/` paths in Phase 11 dir
- `.gitattributes` — needs `*.html linguist-generated=true` addition for HTML reports

### Testing infra
- `Makefile` — `e2e`, `e2e-build`, `e2e-install` targets (Phase 9)
- `frontend/playwright.config.ts` — webServer config + chromium project
- `backend/tests/license_bypass_safety.rs` — exit-code-2 abort contract (referenced as evidence for D-10's mock-hikvision sufficiency argument)

</canonical_refs>

<deferred_ideas>
## Deferred Ideas (out of Phase 11 scope)

These came up during discussion but belong elsewhere. Capturing here to avoid scope creep.

- **Real Hikvision alertStream live test (EVT-01/02 with physical device)** — out of Phase 11 per D-10. Risk-accept doc captured in evidence dir. First production install becomes the live evidence.
- **LIC-05 cross-host clone test on real hardware** — deferred per D-06 to first prod install. Added to v1.1 backlog as `LIC-05-CROSS-HOST` (D-09). Real-customer deploy is the validation gate.
- **Cloud-VM provisioning script** — a reproducibility helper that auto-creates the fresh VM for installer smoke tests. Nice-to-have for v1.1 / when Phase 11 is run a second time after a deploy regression. v1 ships with the operator running a manual provision.
- **Phase 11 re-run automation** — a make target like `make validate-live` that re-runs Phase 11's checks on demand (post-incident or pre-customer-deploy). v1.1 backlog.
- **Evidence dir migration to git-lfs** — if total evidence size grows beyond the 150 MB budget across multiple Phase 11 reruns, migrate `*.html` and `*.png` to git-lfs. Not needed at v1 scale. v1.1 contingent.

</deferred_ideas>

<scope_anchors>
## Scope Anchors (what NOT to do in Phase 11)

- Do NOT modify `backend/src/`, `frontend/src/`, or `.github/workflows/` source code unless evidence runs reveal a regression (D-15).
- Do NOT attempt LIC-05 cross-host on real hardware (D-06).
- Do NOT attempt real Hikvision alertStream test in this phase (D-10).
- Do NOT extend the evidence dir beyond the 5 audit-tracked items (D-13). New "while we're at it" tests belong in a follow-up phase.
- Do NOT skip evidence commits — every PASS must commit before STATE.md updates (D-14).
- Do NOT bypass the gate threshold — if any item shows FAIL, the milestone stays unarchived (D-13). Option is to fix the bug (separate phase or `/gsd-debug` flow), not lower the bar.

</scope_anchors>

<prerequisites>
## Operator Prerequisites Checklist

This is the "block-until-complete" door (D-11). Phase 11 execution starts ONLY after every box is checked:

- [ ] GitHub Actions billing/free-tier budget confirmed for ~5 live runs (~30 min each = ~2.5 hrs of runner time)
- [ ] Fresh Ubuntu 22.04 VM provisioned with Docker, sudo, ~10 GB disk, outbound internet (cloud or local KVM)
- [ ] Valid Cloudflare Zero Trust tunnel TOKEN obtained (pre-create tunnel in CF dashboard, copy token)
- [ ] Valid DO Functions license-activation URL on hand (or a test license JWT pre-issued)
- [ ] CLIENT_SLUG decided (will form `{slug}.cronometrix.com` route)
- [ ] Local dev environment: Rust nightly toolchain installed (per `rust-toolchain.toml`), Node 20, Playwright chromium downloaded
- [ ] Repo state clean: no uncommitted changes; on a Phase 11 working branch (suggest `phase-11-live-validation`)
- [ ] Operator has 2-4 hours uninterrupted

If any item is unchecked, do NOT start Phase 11 execution — the block-until-complete contract requires the full session.

</prerequisites>
