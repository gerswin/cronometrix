---
status: partial
phase: 02-device-integration
source: [02-VERIFICATION.md]
started: 2026-04-20T04:10:00Z
updated: 2026-04-20T04:10:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Hardware smoke — real DS-K1T341 emits an event via alertStream
expected: Register a real Hikvision device, physically perform a face scan, observe exactly one row in `attendance_events` within a few seconds with `employee_id` resolved (or `is_unknown=1`), `direction` set from `attendanceStatus`, `raw_xml` non-empty, and `photo_path` pointing at a saved JPEG under `./data/events/YYYY-MM-DD/`.
result: [pending]

### 2. Reconnect under real network drop
expected: Power-cycle a registered device; observe `WARN stream ended with error` within TCP timeout, followed by `DEBUG reconnect backoff` lines 1000ms → 2000ms → ... capped at 60000ms. On device recovery, `connection_state` flips to `online` and `last_seen_at` refreshes.
result: [pending]

### 3. Dashboard-style real-time feel
expected: GET /api/v1/devices during a real connect/disconnect cycle shows `connection_state` transitions (offline → online → offline) within seconds of the physical event.
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
