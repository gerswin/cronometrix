# Evidence: Real Hikvision alertStream Live Test — Deferral to First Production Install

- **Date captured:** 2026-04-29
- **Captured by:** Phase 11 planner
- **Command run / action:** No live test executed. This document records the formal deferral of the real-hardware Hikvision alertStream end-to-end test per Phase 11 D-10.
- **Expected:** A live test would: (1) connect a real Hikvision DS-K1T341 or DS-K1T342 unit to the network, (2) register it via /api/v1/devices, (3) trigger a face-recognition event on the device, (4) confirm the event lands in attendance_events with correct UTC timestamp + employee mapping + dedup behavior (30-second window per EVT-03).
- **Actual:** Test not executed in Phase 11. The integration is sufficiently exercised by:
  - `backend/src/bin/mock_hikvision.rs` (Phase 9 — impersonates a real Hikvision unit including digest auth + alertStream multipart XML protocol)
  - `frontend/e2e/devices.spec.ts` (Phase 9 — drives the full registration → command-dispatch → audit assertion path against the mock)
  - Phase 2 unit + integration tests covering the alertStream listener, supervisor reconnect loop, and event deduplication

  Real-hardware live test is deferred to first production install because: (a) it requires physical Hikvision hardware in the test environment, (b) the customer's hardware IS the validation environment, (c) ISAPI XML schema variations between Hikvision device models (DS-K1T341 vs DS-K1T342, per Phase 2 blocker note in STATE.md) make synthetic CI hardware an unreliable proxy.

- **Verdict:** DEFERRED — out of Phase 11 scope per D-10. First production install is the live evidence opportunity.

## Risk Acknowledgement

EVT-01/02 risks at first install: a real Hikvision device that uses an ISAPI dialect the mock did not anticipate could fail to establish alertStream, fail digest auth, or produce events the parser does not handle. Mitigations:

1. Phase 2 STATE.md blocker note already flags ISAPI XML schema variation between models — first deploy must capture real alertStream traffic with `tcpdump` or equivalent before declaring EVT-01 green at the customer.
2. Phase 9's `mock_hikvision.rs` is the canonical contract — if the real device produces a payload it does not handle, that's a real-world adapter gap to fix in v1.x, not a Phase 11 PASS gate.
3. v1.1 backlog has the documented opportunity to convert this deferral into a CI-runnable test using a Hikvision-on-loan or virtualized firmware once available.

## Field Test Checklist for First Production Install

Suggested (not mandatory like LIC-05's checklist) on first deploy with real Hikvision hardware:

1. Register device via UI (Devices → Add) with real IP + ISAPI credentials + traffic direction.
2. Watch backend logs for `alertStream connected to {ip}:{port}`.
3. Trigger a face event on the device (have someone approach the camera).
4. Confirm event appears in /api/v1/events within 5 seconds.
5. Trigger a second event from the same employee within 30 seconds — confirm dedup (only 1 row in attendance_events).
6. Disconnect device network for 60s, reconnect — confirm supervisor loop reconnects (per EVT-02).

PASS criteria: all 6 steps green.
FAIL criteria: any step red → file as v1.x adapter bug, NOT as Phase 11 retroactive failure.

- **Artifacts:** None — markdown-only deferral evidence.
- **External refs:**
  - `backend/src/bin/mock_hikvision.rs` (the v1 confidence floor)
  - `frontend/e2e/devices.spec.ts` (E2E coverage of the full path against mock)
  - Phase 2 STATE.md blocker note on ISAPI XML schema variation
  - Phase 11 11-CONTEXT.md D-10 (the binding deferral decision)
