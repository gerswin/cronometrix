---
phase: 2
slug: device-integration
status: draft
nyquist_compliant: true
wave_0_complete: true
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

### Plan 02-03 Task 1 — parser + stream consumer

| Test Name                                          | Location                                              | Requirement |
|----------------------------------------------------|-------------------------------------------------------|-------------|
| deserialize_k1t341_fixture                         | backend/src/isapi/events.rs (unit)                    | EVT-01      |
| strip_xmlns_removes_ver20                          | backend/src/isapi/events.rs (unit)                    | EVT-01      |
| strip_xmlns_removes_ver10                          | backend/src/isapi/events.rs (unit)                    | EVT-01      |
| is_heartbeat_detects_videoloss_inactive            | backend/src/isapi/events.rs (unit)                    | EVT-01      |
| is_heartbeat_detects_explicit_heartbeat            | backend/src/isapi/events.rs (unit)                    | EVT-01      |
| is_heartbeat_false_for_access_event                | backend/src/isapi/events.rs (unit)                    | EVT-01      |
| is_heartbeat_false_for_videoloss_active            | backend/src/isapi/events.rs (unit)                    | EVT-01      |
| direction_mapping_check_in_is_entry                | backend/src/isapi/events.rs (unit)                    | EVT-01      |
| parses_k1t341_fixture_into_one_event_pair          | backend/src/isapi/parser.rs (unit)                    | EVT-01      |
| parses_heartbeat_fixture_into_xml_only_pair        | backend/src/isapi/parser.rs (unit)                    | EVT-01      |
| parses_unknown_face_fixture_into_one_event_pair    | backend/src/isapi/parser.rs (unit)                    | EVT-01      |
| ignores_bytes_before_first_boundary                | backend/src/isapi/parser.rs (unit)                    | EVT-01      |
| fallback_line_scan_if_multer_fails                 | backend/src/isapi/parser.rs (unit)                    | EVT-01      |
| fallback_handles_multiple_events_in_same_buffer    | backend/src/isapi/parser.rs (unit)                    | EVT-01      |
| fallback_returns_empty_for_garbage                 | backend/src/isapi/parser.rs (unit)                    | EVT-01      |
| extract_boundary_multipart_mixed                   | backend/src/isapi/stream.rs (unit)                    | EVT-01      |
| extract_boundary_quoted                            | backend/src/isapi/stream.rs (unit)                    | EVT-01      |
| extract_boundary_form_data                         | backend/src/isapi/stream.rs (unit)                    | EVT-01      |
| extract_boundary_rejects_non_multipart             | backend/src/isapi/stream.rs (unit)                    | EVT-01      |
| connect_and_stream_persists_one_event              | backend/tests/listener_tests.rs                       | EVT-01      |
| heartbeat_updates_last_seen_at_and_does_not_persist| backend/tests/listener_tests.rs                       | DEV-02, A3  |
| unknown_face_persists_with_is_unknown              | backend/tests/listener_tests.rs                       | EVT-03, D-07|
| second_identical_event_deduplicates                | backend/tests/listener_tests.rs                       | EVT-03      |
| connect_and_stream_fails_cleanly_on_401            | backend/tests/listener_tests.rs                       | T-2-05      |
| digest_auth_mock_serves_body_after_challenge       | backend/tests/listener_tests.rs (mock self-test)      | Wave 0      |

### Plan 02-03 Task 2 — supervisor + watchdog + CRUD lifecycle

| Test Name                                          | Location                                              | Requirement |
|----------------------------------------------------|-------------------------------------------------------|-------------|
| bootstrap_spawns_one_task_per_active_device        | backend/tests/supervisor_tests.rs                     | EVT-01      |
| start_signal_spawns_new_task                       | backend/tests/supervisor_tests.rs                     | DEV-04      |
| stop_signal_cancels_task                           | backend/tests/supervisor_tests.rs                     | DEV-04      |
| restart_signal_stops_then_starts                   | backend/tests/supervisor_tests.rs                     | DEV-04      |
| graceful_shutdown_within_5s                        | backend/tests/supervisor_tests.rs                     | EVT-01      |
| watchdog_flips_device_offline_after_90s            | backend/tests/supervisor_tests.rs                     | DEV-02      |
| watchdog_leaves_fresh_device_alone                 | backend/tests/supervisor_tests.rs                     | DEV-02      |
| watchdog_flips_device_with_null_last_seen          | backend/tests/supervisor_tests.rs                     | DEV-02      |
| backoff::doubling_from_initial_caps_at_60s_in_nine_steps | backend/tests/supervisor_tests.rs (unit)       | EVT-02      |
| sleep_ms_with_jitter_within_25_percent             | backend/src/supervisor/task.rs (unit)                 | EVT-02      |
| sleep_ms_with_jitter_handles_small_backoff         | backend/src/supervisor/task.rs (unit)                 | EVT-02      |
| sleep_ms_with_jitter_zero_is_stable                | backend/src/supervisor/task.rs (unit)                 | EVT-02      |
| backoff_cap_reachable_from_initial_in_nine_steps   | backend/src/supervisor/task.rs (unit)                 | EVT-02      |
| post_device_emits_start_event                      | backend/tests/supervisor_tests.rs                     | DEV-04      |
| patch_ip_emits_restart_event                       | backend/tests/supervisor_tests.rs                     | DEV-04      |
| patch_name_only_does_not_emit_restart              | backend/tests/supervisor_tests.rs                     | DEV-04, Pit.7|
| patch_password_emits_restart_event                 | backend/tests/supervisor_tests.rs                     | DEV-04      |
| delete_device_emits_stop_event                     | backend/tests/supervisor_tests.rs                     | DEV-04      |

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
