# Phase 10: v1.0 Documentation & Sign-off Hardening — Context

**Gathered:** 2026-04-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Close every in-repo gap identified by `v1.0-MILESTONE-AUDIT.md` so milestone v1.0 has clean documentation, a refreshed requirements traceability table, the small frontend polish that 09-05 deferred, and a recorded decision on DEPL-03's v1 deferral. Pure documentation + small-feature work — fully autonomous, no external infrastructure required. The `live-environment` follow-ups (CI green/red runs, fresh-VM smoke, LIC-05 cross-host clone) are scoped to Phase 11 and are NOT part of Phase 10.

</domain>

<decisions>
## Implementation Decisions

### Sub-task 1: Post-hoc 01-VERIFICATION.md
- **D-01:** Produce a **full retroactive audit** matching the depth of `09-VERIFICATION.md` (21 must-haves locked with explicit code evidence). Read all 5 of Phase 1's PLAN.md + SUMMARY.md, map every Phase 1 requirement (DATA-01..04, AUTH-01..05, EMP-01..04, DEPT-01..03, RULE-01..03 = 19 REQs) to file:line evidence in the live codebase, and produce a `01-VERIFICATION.md` with frontmatter `status: passed` and a per-requirement table.
- **D-02:** Spawn `gsd-verifier` subagent in worktree isolation (parallel-safe) to do the heavy reading. Orchestrator stays lean. Match the input contract used by recent verifier spawns (phase number + dir + goal + REQ list + files_to_read).
- **D-03:** If the verifier finds genuine evidence gaps (e.g., a REQ's code path can't be located), write the gap as a Phase 11 follow-up item rather than failing the verification. Phase 1 is shipped code; the gap is the missing document, not the missing code.

### Sub-task 2: Post-hoc 07-VERIFICATION.md
- **D-04:** Same approach as D-01..D-03 but for Phase 7 (ENRL-01..05 = 5 REQs + the ISAPI face-upload integration with Phase 2).
- **D-05:** Verifier MUST cross-reference Phase 7's `enrollments/pusher.rs` → `isapi/client.rs` wiring against the integration-checker's matrix in `v1.0-MILESTONE-AUDIT.md` line 8 ("Phase 7 enrollment → Phase 2 ISAPI client") so we don't duplicate evidence-gathering.

### Sub-task 3: REQUIREMENTS.md traceability refresh (full sync)
- **D-06:** Mark all 48 v1 REQs `Complete` in the traceability table where the corresponding phase has shipped (Phases 1, 2, 3, 4, 5, 6, 7). Override stale `Pending` rows for delivered phases (currently 2, 4, 5, 6, 7 are wrongly marked `Pending`).
- **D-07:** Flip the inconsistent `[ ]`/`[x]` checkbox state to match the traceability column. ENRL-* checkboxes are currently `[x]` but the traceability column says `Pending` — bring both into sync.
- **D-08:** Add a new section after the v1 traceability table:
  ```
  ## v1 Cross-Cutting Meta-Requirements (Phases 8+)
  | Requirement | Phase | Status |
  | QUALITY-GATE | Phase 8 | Complete |
  | E2E-TOOLING..E2E-SELECTORS (21 IDs) | Phase 9 | Complete |
  ```
  These meta-requirements track the test-infrastructure investment and weren't part of the original v1 list, so they get their own table rather than back-fitting into the v1 traceability.
- **D-09:** Update DEPL-03's row to:
  ```
  | DEPL-03 | Phase 6 | Partial — accepted v1 ship (D-13 in 06-CONTEXT.md); auto-register strict reading deferred to v1.1 backlog as DEPL-03-AUTO |
  ```
- **D-10:** Update the Coverage block at the bottom (currently says "Mapped to phases: 48") to reflect the v1 + meta total once D-08 lands.

### Sub-task 4: /audit/actors backend endpoint + frontend wiring
- **D-11:** Build a real backend endpoint `GET /api/v1/audit/actors` that returns `[{actor_id: string, username: string, role: string}]` derived from `SELECT DISTINCT al.actor_id, u.username, u.role FROM audit_log al LEFT JOIN users u ON al.actor_id = u.id`. RBAC: same as `/audit` (Admin + Supervisor read; Viewer 403; Anonymous 401). Register in `supervisor_read_routes`.
- **D-12:** No pagination — `audit_log` actor cardinality is bounded (small N: number of users who have ever performed an admin action). One query, one response, no `LIMIT/OFFSET`.
- **D-13:** Frontend: `audit/page.tsx` adds a `useQuery(['audit-actors'], () => api.get('/audit/actors'))` mounted on page load with `staleTime: 5 * 60 * 1000` (5 min cache — actors rarely change). Replace the current `actor_id` raw-string `<option>` values with `{username} ({role})` display + `actor_id` value. Test with the existing 09-05 audit-page Vitest tests + add a new test for the actor-dropdown population.
- **D-14:** Test coverage gate (Phase 8 D-22) applies: backend `audit/handlers.rs` and `audit/service.rs` per-file ≥70%/60%; frontend audit page is in `src/app/**` (excluded from coverage include set per CLAUDE.md), so no new frontend tests required for the page edit, but the existing `__tests__/audit-table.test.tsx` should still pass.
- **D-15:** Bruno collection (`bruno/cronometrix/audit/`) MUST get a new `actors.bru` request to keep parity with the `/audit` endpoint that's already collected.

### Sub-task 5: DEPL-03 final decision record
- **D-16:** Accept the v1 deferral as final. The v1 ship of Phase 6 uses operator-driven Cloudflare Zero Trust (operator pre-creates tunnel in CF dashboard, supplies `CLOUDFLARE_TUNNEL_TOKEN` to installer, installer wires it via the `${TOKEN:?required}` marker and runs `tunnel run`). This is the documented D-13 design choice in `06-CONTEXT.md`.
- **D-17:** No new code in Phase 10 for DEPL-03. The closure is purely documentation.
- **D-18:** Add `DEPL-03-AUTO` to a new `## v1.1 Backlog` section in `REQUIREMENTS.md` with the strict reading: "Installer auto-registers a Cloudflare tunnel by calling the Cloudflare API with a CF API token (not just a tunnel TOKEN), creating the tunnel + DNS route + cloudflared service config in one step." Note that v1.1 may want to evaluate whether `cloudflared tunnel create` CLI invocation suffices, vs the full Go SDK call.
- **D-19:** Update `06-VERIFICATION.md` deferred-items table to cross-reference v1.1 backlog item ID, so future audits trace the deferral.

### Cross-cutting: Phase 10 commit hygiene
- **D-20:** Each sub-task commits atomically (5 commits, mirroring the Phase 9 sub-task commit pattern). Each commit message starts with `docs(10-NN):` for verifications/traceability/DEPL-03 and `feat(10-NN):` for /audit/actors.
- **D-21:** Test gate compliance: backend `cargo nextest run` MUST report ≥757 passing (current baseline) + new audit/actors tests; frontend `npx vitest run` MUST report 337/338 passing (1 pre-existing ActivityFeed failure documented as out-of-scope).
- **D-22:** No new coverage exclusions added to `vitest.config.ts` or `Makefile` regex. CLAUDE.md `## Test Coverage` section's exclusion list stays at its current count.

### Claude's Discretion
- Wave-grouping for execution: Phase 10 has 5 sub-tasks with file-overlap profile suitable for Waves 1+2 split (verifications parallel-safe; traceability + /audit/actors + DEPL-03 doc each modify different files). Planner decides exact wave layout — but if 01-VERIFICATION.md and 07-VERIFICATION.md spawn into worktrees in parallel, the gsd-verifier output for both can land sequentially in one merge window.
- Verifier prompt detail: planner can decide whether to pass `09-VERIFICATION.md` as a structural template explicitly or let the verifier pull it autonomously from `phase_dir` discovery.
- Bruno collection naming: `actors.bru` vs `list-actors.bru` — match the convention already in `bruno/cronometrix/audit/`.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Audit context (drives the entire phase)
- `.planning/v1.0-MILESTONE-AUDIT.md` — full audit with the 5 gaps that scope this phase + integration matrix evidence
- `.planning/REQUIREMENTS.md` — traceability table to refresh; v1 REQ list (48) + needs new meta-REQ section + new v1.1 backlog section
- `.planning/PROJECT.md` — core value statement + on-premise constraint that informs DEPL-03 deferral

### Verification format reference (for retroactive 01/07 VERIFICATIONs)
- `.planning/phases/09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea/09-VERIFICATION.md` — depth/format target (21 must-haves table)
- `.planning/phases/02-device-integration/02-VERIFICATION.md` — alternative format reference (21/21)
- `.planning/phases/03-time-calculation-engine/03-VERIFICATION.md` — minimal-format reference (5/5 passed)

### Phase 1 verification source material
- `.planning/phases/01-foundation/01-00-PLAN.md` through `01-04-PLAN.md` — original plans
- `.planning/phases/01-foundation/01-00-SUMMARY.md` through `01-04-SUMMARY.md` — what shipped
- `.planning/phases/01-foundation/01-CONTEXT.md` — original phase decisions
- `backend/migrations/001_*.sql` and `002_*.sql` — schema evidence for DATA-01..04
- `backend/src/auth/` — JWT login + RBAC middleware for AUTH-01..05
- `backend/src/employees/` + `backend/src/departments/` + `backend/src/rules/` — EMP-01..04, DEPT-01..03, RULE-01..03

### Phase 7 verification source material
- `.planning/phases/07-facial-enrollment-sync/07-01-PLAN.md` and `07-02-PLAN.md`
- `.planning/phases/07-facial-enrollment-sync/07-01-SUMMARY.md` and `07-02-SUMMARY.md`
- `.planning/phases/07-facial-enrollment-sync/07-CONTEXT.md`
- `backend/src/enrollments/` — handlers, service, pusher, image_pipeline, isapi_face
- `backend/src/isapi/client.rs:108,144,233` — upsert_user / upload_face / capture_face_image (per integration-checker evidence)
- `frontend/src/components/enrollment/` — modal + capture tabs
- Migrations 016/017 for face_id / current_face_enrollment_id / state columns

### /audit/actors implementation patterns (mirror existing patterns)
- `backend/src/audit/handlers.rs` — pattern for new actors handler
- `backend/src/audit/service.rs` — pattern for new actors service function
- `backend/src/audit/mod.rs` — pattern for module re-export
- `backend/src/main.rs:237` — registration site for `supervisor_read_routes` (current `/audit` route)
- `backend/src/users/` — for the username/role JOIN target schema
- `frontend/src/app/(dashboard)/audit/page.tsx` — current page that needs the dropdown wiring update
- `frontend/src/components/audit/audit-filters.tsx` — current dropdown component (raw actor_id strings)

### DEPL-03 deferral evidence
- `.planning/phases/06-licensing-deployment/06-CONTEXT.md` D-13 — original design choice rationale
- `.planning/phases/06-licensing-deployment/06-03-SUMMARY.md` — Cloudflare token-based connector implementation
- `.planning/phases/06-licensing-deployment/06-VERIFICATION.md` deferred-items table — current PARTIAL classification

### Project conventions (Phase 8 D-14/D-22 + CLAUDE.md)
- `CLAUDE.md` §Test Coverage — coverage gate thresholds + exclusion policy + per-file floors
- `CLAUDE.md` §Conventions § Filesystem-root injection — `state.paths.*` injection pattern
- `vitest.config.ts` — coverage include/exclude (must remain unchanged in Phase 10)
- `.github/workflows/ci.yml` — `Backend Coverage` + `Frontend Coverage` jobs (must stay green; no new exclusions)

### Test infrastructure (Phase 9 inheritance)
- `backend/tests/audit_handlers_test.rs` — pattern for new actors handler test
- `bruno/cronometrix/audit/` — Bruno collection for the audit module (needs new `actors.bru`)

</canonical_refs>

<deferred_ideas>
## Deferred Ideas (out of Phase 10 scope)

These came up during discussion but belong in other phases or v1.1 backlog. Capturing here so they're not lost.

- **DEPL-03-AUTO (v1.1 backlog):** Strict auto-register interpretation of DEPL-03 — installer calls Cloudflare API with a CF API token to create tunnel + DNS route in one step. v1.1 should also evaluate whether `cloudflared tunnel create` CLI suffices vs full Go SDK.
- **Live-env validation work:** All the manual follow-ups (Phase 8 Plan 05 live PR runs, Phase 9 CI green/red validation, branch protection toggles, fresh-VM installer smoke, LIC-05 cross-host clone test, real Hikvision alertStream test) are scoped to **Phase 11** — explicitly OUT of Phase 10.
- **Audit screen v1.1 polish (not blocking):** Add per-actor avatar/initials in dropdown; date-range presets ("Last 7 days", "This month"); diff-view modal for old_data/new_data instead of inline collapsible. v1.1 backlog.
- **Audit actors caching strategy:** If `/audit/actors` query becomes slow at large audit_log volumes (>1M rows), add a Redis-style cache layer or a denormalized `audit_actors_view`. Not needed at v1 scale; revisit if perf monitoring flags it.

</deferred_ideas>

<scope_anchors>
## Scope Anchors (what NOT to do in Phase 10)

- Do NOT touch `.github/workflows/ci.yml` — Phase 8 + 9 jobs stay byte-identical.
- Do NOT add new coverage exclusions — write tests instead.
- Do NOT spawn integration-checker again — `v1.0-MILESTONE-AUDIT.md` already contains the matrix; reuse its evidence.
- Do NOT implement DEPL-03 auto-register — explicitly deferred to v1.1.
- Do NOT modify Phase 8/9 deliverables — additive only (new endpoint, new docs, refreshed traceability).
- Do NOT run live CI / live VM tests — that's Phase 11.

</scope_anchors>
