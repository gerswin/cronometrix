# Kiosk Capture Flake Remediation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the timing race that made Frontend Coverage intermittently fail after the release-bundle fix reached `main`.

**Architecture:** Keep `KioskCaptureTab` unchanged. At the React Testing Library boundary, await the user-visible `Aceptar` button across the existing mutation, polling query, and effect-driven state transitions.

**Tech Stack:** React 19, TanStack Query v5, Vitest, React Testing Library.

**Spec:** `frontend/src/components/enrollment/__tests__/kiosk-capture-tab.test.tsx`

## Global Constraints

- Do not add sleeps, fake production delays, retries in CI, or production code changes.
- Assert the observable accessible role and name that the user receives.
- Preserve the existing 4-second timeout and the captured Blob assertions.
- Re-run the single test repeatedly before the full frontend coverage gate.

---

### Task 1: Replace the ineffective asynchronous query

**Files:**
- Modify: `frontend/src/components/enrollment/__tests__/kiosk-capture-tab.test.tsx:117`
- Test: `frontend/src/components/enrollment/__tests__/kiosk-capture-tab.test.tsx`

**Interfaces:**
- Consumes: the `Aceptar` button rendered after `KioskCaptureTab` reaches `kioskState === 'captured'`.
- Produces: a `HTMLElement` returned only when React Testing Library observes that accessible button.

- [ ] **Step 1: Confirm the failing behavior**

Use the failed `main` job `97969826746`, which reports `Unable to find an accessible element with the role "button" and name /Aceptar/i` at line 120 after `waitFor(() => queryByRole(...) !== null)` returned without throwing.

- [ ] **Step 2: Implement the minimal wait fix**

```tsx
const acceptBtn = await screen.findByRole(
  'button',
  { name: /Aceptar/i },
  { timeout: 4000 },
)
```

Delete the ineffective `waitFor(() => queryByRole(...) !== null)` and the immediate `getByRole` call.

- [ ] **Step 3: Verify the focused test repeatedly**

Run the test 20 consecutive times:

```bash
cd frontend
for attempt in $(seq 1 20); do
  npx vitest run src/components/enrollment/__tests__/kiosk-capture-tab.test.tsx
done
```

Expected: 20 runs pass, 7 tests per run, with no missing `Aceptar` button.

- [ ] **Step 4: Verify the complete frontend gate**

Run: `make coverage-frontend`

Expected: 68 files and 526 tests pass, followed by the project-wide and per-file coverage gates.

- [ ] **Step 5: Ship the remediation**

Commit, push `fix/kiosk-capture-test-wait`, open a PR to `main`, wait for all required checks, merge, and repeat the post-merge CI verification before publishing the immutable release.
