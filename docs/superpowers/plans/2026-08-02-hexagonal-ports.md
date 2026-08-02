# Hexagonal Ports for Biometric Readers — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put a port between Cronometrix and the Hikvision adapter so a reader from another brand can be added by writing an adapter, not by editing seven application modules.

**Architecture:** Two ports. Inbound, a vendor-neutral `RawMarking` that adapters translate into, with employee resolution and persistence moved to `attendance::ingest`. Outbound, a `BiometricReader` trait that `DeviceConnection` implements and every caller depends on instead of the concrete type. Nothing changes behaviourally — this is the refactor that makes the next vendor cheap.

**Tech Stack:** Rust, Axum 0.8, libSQL/SQLite, `async-trait`, `tokio`, `wiremock` for adapter tests.

## Global Constraints

- Every task ends with `cargo test --all-features` and `cargo clippy --all-targets --all-features` both clean. Neither may regress; the suite is at 1046 passing tests.
- No behavioural change. If a test has to be rewritten rather than moved, stop and say so in the task's commit message — that is a signal the refactor changed semantics.
- The repo requires GSD entry for edits (`CLAUDE.md`), which the user has waived for this work. Do not re-litigate it per task.
- Backend code lives in `backend/`; `cargo` must be run from there.
- Commit after every task. Do not push — the user pushes with `gh` credentials over HTTPS.
- Comments explain WHY, matching the density of the surrounding modules. Do not narrate what the code already says.

## File Structure

| File | Responsibility |
|---|---|
| `backend/src/db/migrations/023_raw_payload_rename.sql` | Rename the column that no longer holds XML |
| `backend/src/attendance/mod.rs` | Module root for vendor-neutral attendance ingestion |
| `backend/src/attendance/marking.rs` | `RawMarking` — the inbound port's data shape |
| `backend/src/attendance/ingest.rs` | Employee resolution + persistence, no vendor types |
| `backend/src/isapi/ingest.rs` | Shrinks to a translator: Hikvision payload → `RawMarking` |
| `backend/src/devices/reader.rs` | `BiometricReader` port + `DeviceCommand` |
| `backend/src/isapi/client.rs` | Gains `impl BiometricReader for DeviceConnection` |

**Out of scope, deliberately:** a `Direction` enum. `RawMarking.direction` stays `Option<String>` carrying the existing `"entry"`/`"exit"` values, because `calc/`, the schema and the API all speak strings and converting them is a separate change with its own blast radius. The vendor-neutrality win is the struct, not the enum.

---

### Task 1: Rename `raw_xml` to `raw_payload`

The column has held JSON since the firmware-compatibility work; the name is now a lie that misleads anyone debugging ingestion. Pure rename, no semantics.

**Files:**
- Create: `backend/src/db/migrations/023_raw_payload_rename.sql`
- Modify: `backend/src/db/mod.rs` (MIGRATIONS array, after the `022_device_ingest_mode` entry)
- Modify: `backend/src/events/models.rs:33`
- Modify: `backend/src/events/service.rs:110,183,195`
- Modify: `backend/src/isapi/ingest.rs:136,151`
- Modify: `backend/src/isapi/stream.rs:436` (doc comment only)
- Test: `backend/src/events/service.rs` (in-module test at line 675)

**Interfaces:**
- Consumes: nothing
- Produces: `NewAttendanceEvent.raw_payload: String` replaces `.raw_xml`; every later task uses the new name.

- [ ] **Step 1: Write the failing test**

Rename the existing round-trip test in `backend/src/events/service.rs` (currently `persist_raw_xml_round_trip` at line 675) and point it at the new column:

```rust
    /// The column holds whatever the device sent — JSON on current firmware —
    /// and must survive byte-for-byte for forensic re-parsing (D-12).
    #[tokio::test]
    async fn persist_raw_payload_round_trip() {
        let (state, tmp) = test_state().await;
        let mut ev = sample_event();
        let payload = r#"{"eventType":"AccessControllerEvent"}"#;
        ev.raw_payload = payload.to_string();

        persist_attendance_event_queued(&state, tmp.path(), ev)
            .await
            .expect("insert");

        let conn = state.db.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT raw_payload FROM attendance_events WHERE id = ?1",
                params!["evt-1".to_string()],
            )
            .await
            .unwrap();
        let stored: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(stored, payload, "raw_payload must round-trip byte-for-byte");
    }
```

Read the surrounding test module first — `test_state()`, `sample_event()` and the event id used at line 675 already exist there. Reuse them exactly rather than inventing new helpers.

- [ ] **Step 2: Run test to verify it fails**

```bash
cd backend && cargo test --lib events::service::tests::persist_raw_payload_round_trip
```

Expected: compile error — `no field raw_payload on type NewAttendanceEvent`.

- [ ] **Step 3: Write the migration**

Create `backend/src/db/migrations/023_raw_payload_rename.sql`:

```sql
-- 023_raw_payload_rename.sql
-- The column has not held XML since firmware V3.3.8 support landed: DS-K1T341CMFW
-- pushes JSON, and `isapi::parser` now accepts either. A column called `raw_xml`
-- full of JSON is a trap for whoever debugs ingestion next.
--
-- Rename only. The contents are untouched, and the read-side mappers still omit
-- the column from `EVENT_SELECT_COLS` (T-2-14) — raw device payloads are kept for
-- forensics and never exposed on the API.
--
-- `ALTER TABLE ... RENAME COLUMN` needs SQLite 3.25+; libSQL is well past that.
-- No audit trigger references attendance_events, so nothing else has to move.

ALTER TABLE attendance_events RENAME COLUMN raw_xml TO raw_payload;
```

Register it in `backend/src/db/mod.rs`, immediately after the `022_device_ingest_mode` tuple:

```rust
    (
        "023_raw_payload_rename",
        include_str!("migrations/023_raw_payload_rename.sql"),
    ),
```

- [ ] **Step 4: Rename the field and its uses**

In `backend/src/events/models.rs`, rename the struct field and correct the doc comment above the struct (line 4) that calls it "raw XML":

```rust
    /// Verbatim device payload — JSON on current firmware, XML on older units.
    /// Kept for forensic re-parsing per D-12 and never exposed on the API.
    pub raw_payload: String,
```

In `backend/src/events/service.rs`: update the `EVENT_SELECT_COLS` doc comment at line 110 to say `raw_payload`, the INSERT column list at line 183, and the bound parameter at line 195.

In `backend/src/isapi/ingest.rs`: lines 136 and 151 construct `NewAttendanceEvent`; change `raw_xml:` to `raw_payload:`.

In `backend/src/isapi/stream.rs:436`, the `PendingAlert.raw` doc comment says the schema calls it `raw_xml`. It no longer does — delete that sentence, keeping the rest.

- [ ] **Step 5: Fix the test files that construct the struct**

These construct `NewAttendanceEvent` and will not compile until updated. Change `raw_xml:` to `raw_payload:` in each:

```
backend/tests/events_handlers_extra_test.rs
backend/tests/event_tests.rs
backend/tests/daily_records_service_test.rs
backend/tests/leave_tests.rs
backend/tests/listener_tests.rs
backend/tests/daily_record_tests.rs
backend/tests/events_service_edges_test.rs
```

Find them with `grep -rln raw_xml backend/tests/`. Any that assert against the column name in SQL need the same change.

- [ ] **Step 6: Run the full suite**

```bash
cd backend && cargo test --all-features 2>&1 | grep -E "^test result|FAILED"
cd backend && cargo clippy --all-targets --all-features 2>&1 | grep -E "^(error|warning)"
```

Expected: 1046+ passing, 0 failed, clippy silent.

- [ ] **Step 7: Verify the migration applies to the live dev database**

```bash
cd backend && ./target/debug/cronometrix 2>&1 | grep -m1 "023_raw_payload_rename"
```

Expected: `Applied migration: 023_raw_payload_rename`. Stop the process afterwards. If port 3001 is busy, another backend is running — stop that one first.

- [ ] **Step 8: Commit**

```bash
git add backend/src/db/migrations/023_raw_payload_rename.sql backend/src/db/mod.rs \
        backend/src/events/models.rs backend/src/events/service.rs \
        backend/src/isapi/ingest.rs backend/src/isapi/stream.rs backend/tests/
git commit -m "refactor(db): rename raw_xml to raw_payload

The column has held JSON since firmware V3.3.8 support landed; a column called
raw_xml full of JSON is a trap for whoever debugs ingestion next. Rename only —
contents untouched, and the read-side mappers still omit it (T-2-14)."
```

---

### Task 2: Extract `RawMarking` and a vendor-free ingest

`isapi::ingest::ingest_alert` currently does two jobs: decode a Hikvision payload, and resolve/persist a marking. The second half is domain logic living in a vendor module and keyed on a vendor type, so a second adapter cannot reuse it without fabricating a fake `EventNotificationAlert`.

**Files:**
- Create: `backend/src/attendance/mod.rs`
- Create: `backend/src/attendance/marking.rs`
- Create: `backend/src/attendance/ingest.rs`
- Modify: `backend/src/lib.rs` (add `pub mod attendance;`)
- Modify: `backend/src/isapi/ingest.rs` (becomes a translator)
- Test: `backend/src/attendance/ingest.rs` (in-module) and existing `backend/tests/device_push_test.rs` must keep passing untouched

**Interfaces:**
- Consumes: `NewAttendanceEvent.raw_payload` from Task 1.
- Produces:
  - `attendance::marking::RawMarking { external_person_id: Option<String>, face_id: Option<String>, occurred_at: i64, direction: Option<String>, photo: Option<Vec<u8>>, raw_payload: String }`
  - `attendance::ingest::ingest(state: &AppState, device_id: &str, direction_default: &str, marking: RawMarking) -> anyhow::Result<IngestOutcome>`
  - `attendance::ingest::IngestOutcome { Persisted, Deduplicated, Skipped }` — moved from `isapi::ingest`, which re-exports it so existing imports keep compiling.

- [ ] **Step 1: Write the failing test**

Create `backend/src/attendance/ingest.rs` with only this test module at first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The domain ingest must resolve and persist a marking without ever seeing
    /// a vendor payload — that is the whole point of the port.
    #[tokio::test]
    async fn persists_a_marking_and_resolves_the_employee() {
        let (state, _tmp) = crate::test_support::state_with_seeded_employee().await;

        let outcome = ingest(
            &state,
            "dev-1",
            "entry",
            RawMarking {
                external_person_id: Some("EMP-1".into()),
                face_id: None,
                occurred_at: 1_785_000_000,
                direction: Some("exit".into()),
                photo: None,
                raw_payload: "{}".into(),
            },
        )
        .await
        .expect("ingest");

        assert_eq!(outcome, IngestOutcome::Persisted);
    }

    /// A marking that names nobody is a door or tamper notification, not
    /// attendance. Persisting it would invent an unknown-face row every time the
    /// door moved.
    #[tokio::test]
    async fn skips_a_marking_with_no_identity() {
        let (state, _tmp) = crate::test_support::state_with_seeded_employee().await;

        let outcome = ingest(
            &state,
            "dev-1",
            "entry",
            RawMarking {
                external_person_id: None,
                face_id: None,
                occurred_at: 1_785_000_000,
                direction: None,
                photo: None,
                raw_payload: "{}".into(),
            },
        )
        .await
        .expect("ingest");

        assert_eq!(outcome, IngestOutcome::Skipped);
    }
}
```

`crate::test_support::state_with_seeded_employee` does not exist. Before writing it, check whether an in-crate test helper already builds an `AppState` with a tempdir — `backend/tests/common/mod.rs` has `test_state_with_tmpdir` but that is a test-only crate, not importable from `src/`. If no in-crate helper exists, write these two tests as an integration test in `backend/tests/attendance_ingest_test.rs` instead, reusing `common::test_state_with_tmpdir` and the seeding pattern from `backend/tests/device_push_test.rs`. Prefer that — do not add a `test_support` module to `src/` just for this.

- [ ] **Step 2: Run test to verify it fails**

```bash
cd backend && cargo test --test attendance_ingest_test
```

Expected: FAIL — `attendance` module does not exist.

- [ ] **Step 3: Write `RawMarking`**

Create `backend/src/attendance/marking.rs`:

```rust
//! The inbound port's data shape.
//!
//! Adapters translate their vendor payload into this and hand it to
//! `attendance::ingest`. Nothing here names a manufacturer, a protocol or a
//! wire format — that is what lets a second reader brand reuse the resolution
//! and persistence logic instead of reimplementing it.

/// One authentication a reader reported, stripped of vendor framing.
pub struct RawMarking {
    /// The identifier the device knows this person by. On Hikvision this is
    /// whatever was pushed as `UserInfo.employeeNo`, echoed back in
    /// `employeeNoString`; other vendors use other fields.
    pub external_person_id: Option<String>,
    /// A separate face-library identifier, when the device reports one apart
    /// from the person id. Both are tried against `device_face_mappings`.
    pub face_id: Option<String>,
    /// UTC epoch seconds. Adapters resolve their own clock format; the domain
    /// never parses a device timestamp.
    pub occurred_at: i64,
    /// `"entry"` or `"exit"` when the reader reported one. `None` means the
    /// device did not say, and the device's configured default applies.
    pub direction: Option<String>,
    pub photo: Option<Vec<u8>>,
    /// Verbatim payload, kept for forensic re-parsing (D-12).
    pub raw_payload: String,
}

impl RawMarking {
    /// Whether this names somebody.
    ///
    /// Readers interleave door, tamper and status notifications with real
    /// markings; only the latter carry an identity.
    pub fn has_identity(&self) -> bool {
        self.external_person_id
            .as_deref()
            .is_some_and(|value| !value.is_empty())
            || self.face_id.as_deref().is_some_and(|value| !value.is_empty())
    }
}
```

- [ ] **Step 4: Move resolution and persistence into `attendance::ingest`**

Write `backend/src/attendance/ingest.rs` above the test module. Move the body of `isapi::ingest::ingest_alert` from the `has_identity()` check onward, verbatim, replacing every `ace.` / `alert.` access with the corresponding `marking.` field:

```rust
//! Vendor-neutral attendance ingestion.
//!
//! Everything from identity resolution onward: which employee a marking belongs
//! to, whether it is worth storing, and how it is persisted. Adapters supply a
//! `RawMarking`; nothing in here knows which protocol produced it.

use crate::events::models::{NewAttendanceEvent, PersistOutcome};
use crate::events::service as events_service;
use crate::state::AppState;

use super::marking::RawMarking;

/// What ingesting one marking did, so callers can log or count without
/// re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOutcome {
    Persisted,
    Deduplicated,
    /// Named nobody, or was a heartbeat. Deliberately not persisted.
    Skipped,
}

pub async fn ingest(
    state: &AppState,
    device_id: &str,
    direction_default: &str,
    marking: RawMarking,
) -> anyhow::Result<IngestOutcome> {
    if !marking.has_identity() {
        tracing::debug!(device_id = %device_id, "marking without an identity — skipped");
        return Ok(IngestOutcome::Skipped);
    }
    // ... the rest is the existing body from isapi::ingest, with:
    //   captured_at        <- marking.occurred_at
    //   direction          <- marking.direction.unwrap_or(direction_default)
    //   face_id            <- marking.face_id
    //   employee_no_string <- marking.external_person_id
    //   raw_payload        <- marking.raw_payload
    //   photo_bytes        <- marking.photo
}
```

Do not retype the persistence block from memory — copy it out of `isapi::ingest.rs` so the `sse_snapshot`, `publish_recompute_if_employee` and `publish_sse_event` calls stay exactly as they are. Getting the post-commit ordering wrong here silently breaks the dashboard.

Create `backend/src/attendance/mod.rs`:

```rust
pub mod ingest;
pub mod marking;
```

Add `pub mod attendance;` to `backend/src/lib.rs`, keeping the module list alphabetical if it already is.

- [ ] **Step 5: Reduce `isapi::ingest` to a translator**

`ingest_alert` keeps its signature so `stream.rs` and `push.rs` need no changes. Its body becomes: trace the payload, parse, drop heartbeats, build a `RawMarking`, delegate.

```rust
    let ace = match alert.access_controller_event.as_ref() {
        Some(ace) => ace,
        None => {
            tracing::debug!(device_id = %device_id, "alert without AccessControllerEvent — skipped");
            return Ok(IngestOutcome::Skipped);
        }
    };

    let marking = RawMarking {
        external_person_id: (!ace.employee_no_string.is_empty())
            .then(|| ace.employee_no_string.clone()),
        face_id: (!ace.face_id.is_empty()).then(|| ace.face_id.clone()),
        occurred_at: alert
            .captured_at_epoch()
            .unwrap_or_else(|| chrono::Utc::now().timestamp()),
        direction: ace.reported_direction().map(str::to_string),
        photo: jpeg_bytes.map(|bytes| bytes.to_vec()),
        raw_payload,
    };

    attendance::ingest::ingest(state, device_id, direction_default, marking).await
```

Keep the `sub`/`major` detail in the skip log — move that logging into the translator, since only the adapter knows those codes:

```rust
    if !ace.has_identity() {
        tracing::debug!(
            device_id = %device_id,
            major = ?ace.major_event_type,
            sub = ?ace.sub_event_type,
            device_time = %alert.date_time,
            verify_mode = %ace.current_verify_mode,
            attendance_status = %ace.attendance_status,
            has_photo = jpeg_bytes.is_some(),
            "access-control event without an identity — skipped"
        );
        return Ok(IngestOutcome::Skipped);
    }
```

Re-export the outcome so existing `use crate::isapi::ingest::{ingest_alert, IngestOutcome}` in `devices/push.rs` still resolves:

```rust
pub use crate::attendance::ingest::IngestOutcome;
```

- [ ] **Step 6: Run the full suite**

```bash
cd backend && cargo test --all-features 2>&1 | grep -E "^test result|FAILED"
cd backend && cargo clippy --all-targets --all-features 2>&1 | grep -E "^(error|warning)"
```

Expected: all green. `backend/tests/device_push_test.rs` must pass **unmodified** — if it needed changes, the refactor altered behaviour.

- [ ] **Step 7: Commit**

```bash
git add backend/src/attendance/ backend/src/lib.rs backend/src/isapi/ingest.rs backend/tests/attendance_ingest_test.rs
git commit -m "refactor(attendance): put a vendor-neutral port in front of ingestion

isapi::ingest did two jobs: decode a Hikvision payload, and resolve and persist
a marking. The second is domain logic that lived in a vendor module and was
keyed on a vendor type, so a second adapter could not reuse it without
fabricating a fake EventNotificationAlert.

Adapters now translate into RawMarking and delegate. No behavioural change --
device_push_test.rs passes unmodified."
```

---

### Task 3: Define the `BiometricReader` port

`DeviceConnection` is a concrete struct with 17 public methods, constructed in seven modules outside `isapi/`. A second vendor means editing all seven.

**Files:**
- Create: `backend/src/devices/reader.rs`
- Modify: `backend/src/devices/mod.rs` (add `pub mod reader;`)
- Modify: `backend/src/isapi/client.rs` (add the impl block)
- Modify: `backend/Cargo.toml` (add `async-trait` if absent — check first)
- Test: `backend/tests/biometric_reader_test.rs`

**Interfaces:**
- Consumes: nothing from Tasks 1–2.
- Produces:
  - `devices::reader::DeviceCommand { DoorOpen, Reboot, EnrollmentMode }`
  - `devices::reader::ProvisioningIntent { local_time: String, time_zone: String, require_direction: bool, day_split: String }`
  - `devices::reader::ProvisionReport { applied: Vec<&'static str>, unsupported: Vec<&'static str>, failed: Vec<String> }`
  - `#[async_trait] devices::reader::BiometricReader` with `provision`, `enroll`, `revoke`, `capture_face`, `execute` — Task 4 migrates callers onto it.

- [ ] **Step 1: Check whether `async-trait` is already a dependency**

```bash
cd backend && grep -n "async-trait" Cargo.toml || echo "ABSENT"
```

If absent, add `async-trait = "0.1"` to `[dependencies]`. Do not add any other crate.

- [ ] **Step 2: Write the failing test**

Create `backend/tests/biometric_reader_test.rs`:

```rust
//! The port exists so application code can depend on a capability rather than a
//! manufacturer. This test never names Hikvision: it drives a fake reader,
//! which is exactly what a second vendor's adapter has to satisfy.

use async_trait::async_trait;
use cronometrix_api::devices::reader::{
    BiometricReader, DeviceCommand, ProvisionReport, ProvisioningIntent,
};

struct FakeReader {
    enrolled: std::sync::Mutex<Vec<String>>,
}

#[async_trait]
impl BiometricReader for FakeReader {
    async fn provision(&self, intent: &ProvisioningIntent) -> anyhow::Result<ProvisionReport> {
        // A reader that cannot honour a request reports it rather than lying.
        let mut report = ProvisionReport::default();
        report.applied.push("clock");
        if intent.require_direction {
            report.unsupported.push("attendance_mode");
        }
        Ok(report)
    }

    async fn enroll(&self, person_id: &str, _face: &[u8]) -> anyhow::Result<()> {
        self.enrolled.lock().unwrap().push(person_id.to_string());
        Ok(())
    }

    async fn revoke(&self, person_id: &str) -> anyhow::Result<()> {
        self.enrolled.lock().unwrap().retain(|id| id != person_id);
        Ok(())
    }

    async fn capture_face(&self) -> anyhow::Result<Vec<u8>> {
        Ok(vec![0xFF, 0xD8, 0xFF])
    }

    async fn execute(&self, _command: DeviceCommand) -> anyhow::Result<String> {
        Ok("ok".to_string())
    }
}

#[tokio::test]
async fn a_reader_reports_what_it_could_not_apply_instead_of_failing_silently() {
    let reader = FakeReader {
        enrolled: std::sync::Mutex::new(Vec::new()),
    };
    let report = reader
        .provision(&ProvisioningIntent {
            local_time: "2026-08-02T13:00:00".into(),
            time_zone: "CST+4:00:00".into(),
            require_direction: true,
            day_split: "13:00:00".into(),
        })
        .await
        .expect("provision");

    assert_eq!(report.applied, vec!["clock"]);
    assert_eq!(
        report.unsupported,
        vec!["attendance_mode"],
        "an unsupported capability must be visible to the caller"
    );
}

#[tokio::test]
async fn enrol_and_revoke_round_trip_through_the_port() {
    let reader = FakeReader {
        enrolled: std::sync::Mutex::new(Vec::new()),
    };
    reader.enroll("person-1", &[0xFF, 0xD8, 0xFF]).await.unwrap();
    assert_eq!(reader.enrolled.lock().unwrap().len(), 1);
    reader.revoke("person-1").await.unwrap();
    assert!(reader.enrolled.lock().unwrap().is_empty());
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cd backend && cargo test --test biometric_reader_test
```

Expected: FAIL — `devices::reader` does not exist.

- [ ] **Step 4: Write the port**

Create `backend/src/devices/reader.rs`:

```rust
//! The outbound port: what Cronometrix needs a biometric reader to do.
//!
//! Application code depends on this rather than on a manufacturer's client, so
//! adding a brand means writing an adapter instead of editing every caller.
//!
//! `provision` takes an INTENT, not orders. Readers differ in what they can
//! express — schedules, function keys, picture upload — and a caller that
//! issued vendor-specific instructions would be an adapter in disguise. The
//! report exists because this hardware answers `statusCode 1` to writes it does
//! not apply: an adapter must be able to say "I could not do that" without
//! either lying or failing the whole operation.

use async_trait::async_trait;

/// A one-shot instruction with no vendor semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceCommand {
    DoorOpen,
    Reboot,
    EnrollmentMode,
}

/// What the installation needs of a reader, in domain terms.
pub struct ProvisioningIntent {
    /// Local wall-clock time, `%Y-%m-%dT%H:%M:%S`.
    pub local_time: String,
    /// POSIX `TZ` string. The sign is inverted relative to the ISO offset.
    pub time_zone: String,
    /// Whether every marking must carry a direction. A reader that cannot
    /// guarantee it reports `attendance_mode` as unsupported.
    pub require_direction: bool,
    /// Midpoint splitting arrivals from departures when the reader infers them.
    pub day_split: String,
}

/// What an adapter managed to apply.
///
/// Returned instead of `()` so a partially-provisioned reader is visible.
#[derive(Debug, Default)]
pub struct ProvisionReport {
    pub applied: Vec<&'static str>,
    /// Capabilities this hardware does not have. Not an error.
    pub unsupported: Vec<&'static str>,
    /// Capabilities it should have honoured and did not.
    pub failed: Vec<String>,
}

#[async_trait]
pub trait BiometricReader: Send + Sync {
    async fn provision(&self, intent: &ProvisioningIntent) -> anyhow::Result<ProvisionReport>;
    /// `person_id` is the identifier the device will report back on a marking.
    async fn enroll(&self, person_id: &str, face: &[u8]) -> anyhow::Result<()>;
    async fn revoke(&self, person_id: &str) -> anyhow::Result<()>;
    async fn capture_face(&self) -> anyhow::Result<Vec<u8>>;
    async fn execute(&self, command: DeviceCommand) -> anyhow::Result<String>;
}
```

Add `pub mod reader;` to `backend/src/devices/mod.rs`.

- [ ] **Step 5: Run test to verify it passes**

```bash
cd backend && cargo test --test biometric_reader_test
```

Expected: 2 passed.

- [ ] **Step 6: Implement the port for `DeviceConnection`**

Append to `backend/src/isapi/client.rs`. Delegate to the existing inherent methods; do not move their bodies.

```rust
#[async_trait::async_trait]
impl crate::devices::reader::BiometricReader for DeviceConnection {
    async fn provision(
        &self,
        intent: &crate::devices::reader::ProvisioningIntent,
    ) -> Result<crate::devices::reader::ProvisionReport> {
        let mut report = crate::devices::reader::ProvisionReport::default();

        match self.set_time(&intent.local_time, &intent.time_zone).await {
            Ok(_) => report.applied.push("clock"),
            Err(error) => report.failed.push(format!("clock: {error}")),
        }
        // Remaining steps follow the same shape. Copy the exact call sequence
        // and ordering from `isapi::stream::provision_device` — the webhook
        // slots in particular MUST be cleared before ours is written, or this
        // firmware recompacts the list and drops the address.
        Ok(report)
    }

    async fn enroll(&self, person_id: &str, face: &[u8]) -> Result<()> {
        self.upsert_user(person_id, person_id).await?;
        self.upload_face(person_id, face.to_vec()).await?;
        Ok(())
    }

    async fn revoke(&self, person_id: &str) -> Result<()> {
        self.delete_user(person_id).await?;
        Ok(())
    }

    async fn capture_face(&self) -> Result<Vec<u8>> {
        self.capture_face_image().await
    }

    async fn execute(&self, command: crate::devices::reader::DeviceCommand) -> Result<String> {
        use crate::devices::reader::DeviceCommand;
        match command {
            DeviceCommand::DoorOpen => self.door_open().await,
            DeviceCommand::Reboot => self.reboot().await,
            DeviceCommand::EnrollmentMode => self.enrollment_mode().await,
        }
    }
}
```

`enroll` passing `person_id` as the display name is wrong — check `enrollments::pusher` for what it currently passes as `full_name` and add a `display_name: &str` parameter to the trait method if it passes something different. Update the fake in the test to match.

- [ ] **Step 7: Run the full suite**

```bash
cd backend && cargo test --all-features 2>&1 | grep -E "^test result|FAILED"
cd backend && cargo clippy --all-targets --all-features 2>&1 | grep -E "^(error|warning)"
```

- [ ] **Step 8: Commit**

```bash
git add backend/Cargo.toml backend/src/devices/reader.rs backend/src/devices/mod.rs \
        backend/src/isapi/client.rs backend/tests/biometric_reader_test.rs
git commit -m "feat(devices): define the BiometricReader port

Application code has depended on the concrete DeviceConnection, so a second
brand would mean editing seven modules. provision takes an intent rather than
orders, and reports what it could not apply -- this hardware answers
statusCode 1 to writes it does not honour, so an adapter needs a way to say so
without lying or failing outright."
```

---

### Task 4: Migrate callers onto the port

**Files:**
- Modify: `backend/src/devices/handlers.rs` (command dispatch)
- Modify: `backend/src/workers/purge.rs` (revoke)
- Modify: `backend/src/enrollments/pusher.rs` (enroll)
- Modify: `backend/src/enrollments/handlers.rs` (capture)
- Modify: `backend/src/enrollments/service.rs`
- Modify: `backend/src/isapi/stream.rs` (`provision_device` delegates to the port)
- Test: existing suites must pass unmodified

**Interfaces:**
- Consumes: the whole of Task 3.
- Produces: no new API. After this, `DeviceConnection` is named only in `isapi/` and in the factory.

- [ ] **Step 1: Add the factory**

In `backend/src/devices/reader.rs`:

```rust
/// Build the adapter for a device.
///
/// The only place outside `isapi/` that names a manufacturer. When
/// `devices.vendor` exists this dispatches on it; until then every reader is
/// Hikvision, and saying so in one function beats saying it in seven.
pub fn reader_for(
    base_url: &str,
    username: &str,
    password: &str,
    allow_insecure_tls: bool,
) -> anyhow::Result<Box<dyn BiometricReader>> {
    Ok(Box::new(crate::isapi::client::DeviceConnection::new(
        base_url,
        username,
        password,
        allow_insecure_tls,
    )?))
}
```

- [ ] **Step 2: Migrate one caller and run its tests**

Start with `backend/src/workers/purge.rs` — it uses a single method (`delete_user`) and has the smallest blast radius. Replace `DeviceConnection::new(...)` with `reader_for(...)` and `.delete_user(face_id)` with `.revoke(face_id)`.

```bash
cd backend && cargo test --test purge_worker_test 2>&1 | grep -E "^test result|FAILED"
```

Find the actual test file name with `ls backend/tests | grep -i purge` if that guess is wrong.

- [ ] **Step 3: Commit that caller**

```bash
git add backend/src/devices/reader.rs backend/src/workers/purge.rs
git commit -m "refactor(purge): depend on the BiometricReader port"
```

- [ ] **Step 4: Migrate the remaining callers, one commit each**

In this order, smallest first. Run the full suite after each and commit before moving on:

1. `devices/handlers.rs` — `door_open`/`reboot`/`enrollment_mode` become `execute(DeviceCommand::*)`. The `Command` enum in `devices/models.rs` maps onto `DeviceCommand`; keep the existing enum as the API-facing type and convert at the handler boundary.
2. `enrollments/handlers.rs` — `capture_face_image()` becomes `capture_face()`.
3. `enrollments/pusher.rs` — `upsert_user` + `upload_face` become one `enroll()`. Read carefully: the pusher treats `duplicateEmployeeNo` as success and has checkpoint semantics around the two calls. If collapsing them into `enroll()` would change when a checkpoint is written, leave the pusher on `DeviceConnection` and note why in the commit message.
4. `enrollments/service.rs`.
5. `isapi/stream.rs::provision_device` — delegates to `BiometricReader::provision`, building the `ProvisioningIntent` from the existing constants and logging the returned report.

- [ ] **Step 5: Verify the coupling actually dropped**

```bash
cd backend && grep -rn "DeviceConnection" src/ --include=*.rs | grep -v "^src/isapi/" | grep -v "reader.rs"
```

Expected: no output, or only sites documented in Step 4 as deliberately left behind.

- [ ] **Step 6: Update the audit document**

In `docs/ARQUITECTURA-HEXAGONAL.md`, mark steps 1–3 of the migration path as done, with the commit SHAs, and move the remaining items (`devices.vendor` column, non-HTTP connection model) into a "still open" section. Do not rewrite the findings — they are the record of why this work happened.

- [ ] **Step 7: Commit**

```bash
git add backend/src docs/ARQUITECTURA-HEXAGONAL.md
git commit -m "refactor(devices): move every caller onto the BiometricReader port

DeviceConnection is now named only inside isapi/ and the factory. Adding a
second brand is an adapter plus a match arm."
```

---

## Self-Review

**Spec coverage** — the audit's migration path has five steps. Steps 1–3 map to Tasks 1–4 here. Steps 4 (`devices.vendor` column) and 5 (non-HTTP connection model) are deliberately excluded: the audit itself says step 5 should wait for a real non-HTTP device, and step 4 is a one-line migration best done when the second adapter lands and its dispatch arm can be tested. Task 4 Step 6 records both as still open.

**Placeholder scan** — Task 3 Step 6 and Task 4 Step 4 describe call sequences rather than pasting them. That is deliberate and flagged inline: both must be copied from existing code (`provision_device`, the pusher's checkpoint handling) because retyping them from memory is how the webhook-slot ordering bug would return. Every other step carries its actual content.

**Type consistency** — `RawMarking` fields are used identically in Task 2 Steps 1, 3 and 5. `IngestOutcome` is defined once in `attendance::ingest` and re-exported from `isapi::ingest`. `ProvisionReport` uses `applied`/`unsupported`/`failed` in both the trait definition and the fake. Task 3 Step 6 flags one real uncertainty — whether `enroll` needs a `display_name` parameter — and says how to resolve it rather than guessing.
