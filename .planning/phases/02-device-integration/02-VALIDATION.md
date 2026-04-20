---
phase: 2
slug: device-integration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-19
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test / cargo nextest (Rust) |
| **Config file** | backend/Cargo.toml (dev-dependencies) |
| **Quick run command** | `cd backend && cargo nextest run --no-fail-fast --test-threads=4` |
| **Full suite command** | `cd backend && cargo nextest run --all-features` |
| **Estimated runtime** | ~30 seconds (unit + integration with in-memory libSQL) |

---

## Sampling Rate

- **After every task commit:** Run quick command for the modified crate module
- **After every plan wave:** Run full suite command
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

### Plan 02-02 Task 1 — persist helper + Wave 0 fixtures

| Test Name                                     | Location                                              | Requirement |
|-----------------------------------------------|-------------------------------------------------------|-------------|
| persist_dedup_within_30s                      | backend/src/events/service.rs (unit)                  | EVT-03      |
| persist_cross_device_within_30s               | backend/src/events/service.rs (unit, D-05)            | EVT-03      |
| persist_adjacent_buckets                      | backend/src/events/service.rs (unit)                  | EVT-03      |
| persist_epoch_is_utc_integer                  | backend/src/events/service.rs (unit)                  | EVT-04      |
| persist_raw_xml_round_trip                    | backend/src/events/service.rs (unit, D-12)            | EVT-04      |
| persist_unknown_face_sets_is_unknown          | backend/src/events/service.rs (unit, D-07)            | EVT-03      |
| persist_photo_written_on_insert               | backend/src/events/service.rs (unit, D-13)            | EVT-03      |
| persist_photo_skipped_on_dedup                | backend/src/events/service.rs (unit, D-13)            | EVT-03      |
| mock_hikvision_serves_canned_body             | backend/tests/common/mock_hikvision.rs (smoke)        | Wave 0      |
| fixture_k1t341_exists_and_contains_event_xml  | backend/tests/common/mock_hikvision.rs (presence)     | Wave 0      |
| fixture_heartbeat_exists_and_contains_marker  | backend/tests/common/mock_hikvision.rs (presence)     | Wave 0      |
| fixture_unknown_face_has_face_id              | backend/tests/common/mock_hikvision.rs (presence)     | Wave 0      |

### Plan 02-02 Task 2 — read API (filled in at end of Task 2)

| Test Name                                  | Location                      | Requirement |
|--------------------------------------------|-------------------------------|-------------|
| list_events_empty_returns_empty_array      | backend/tests/event_tests.rs  | DEV-02      |
| list_events_pagination_clamps_limit        | backend/tests/event_tests.rs  | DEV-02      |
| list_events_filters_by_employee_id         | backend/tests/event_tests.rs  | DEV-02      |
| list_events_filters_by_device_id           | backend/tests/event_tests.rs  | DEV-02      |
| list_events_filters_by_time_range          | backend/tests/event_tests.rs  | DEV-02      |
| list_events_viewer_can_read                | backend/tests/event_tests.rs  | DEV-02, D-15|
| list_events_unauthenticated_401            | backend/tests/event_tests.rs  | DEV-02      |
| get_event_by_id_404_if_missing             | backend/tests/event_tests.rs  | DEV-02      |
| get_event_photo_returns_jpeg_bytes         | backend/tests/event_tests.rs  | DEV-02      |
| get_event_photo_404_if_no_photo_path       | backend/tests/event_tests.rs  | DEV-02      |
| get_event_photo_404_if_file_missing        | backend/tests/event_tests.rs  | DEV-02      |
| get_event_photo_rejects_path_traversal     | backend/tests/event_tests.rs  | T-2-06      |

Plan 02-03 tests (supervisor, reconnect, parser) remain unmapped — that plan completes Phase 2 validation. `nyquist_compliant` stays false. `wave_0_complete` flips true after 02-03 lands the digest-auth mock variant and a real-device hardware smoke.

---

## Wave 0 Requirements

- [ ] `backend/tests/fixtures/mock_hikvision.rs` — tokio TCP server fixture that serves canned multipart/mixed alertStream bytes
- [ ] `backend/tests/fixtures/alertstream_samples/` — pcap/bytes samples from DS-K1T341 / DS-K1T342 (flagged BLOCKER per STATE.md)
- [ ] `backend/tests/common/mod.rs` — shared test harness (spin up Axum with in-memory libSQL, seeded test users, test DEVICE_CREDS_KEY)
- [ ] cargo-nextest added to dev tooling if not present

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real device handshake + digest auth + multipart parse against DS-K1T341 | DEV-03 / EVT-01 | Requires physical hardware; fixture cannot fully validate firmware quirks | Register a real device, trigger face-scan, verify event lands in DB with correct employee_id and raw_xml |
| Door open command on physical access controller | DEV-04 | Side effect on physical door; not safe in automated test | POST /api/v1/devices/:id/commands {command:"door_open"} and confirm door opens within 10s |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
