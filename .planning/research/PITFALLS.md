# Pitfalls Research

**Domain:** Biometric Time & Attendance System (Hikvision ISAPI, Rust/Axum, SQLite+Turso, hybrid on-premise)
**Researched:** 2026-04-11
**Confidence:** MEDIUM-HIGH (domain-specific findings verified across multiple sources; some Hikvision internals LOW confidence due to sparse public docs)

---

## Critical Pitfalls

### Pitfall 1: Hikvision Event Stream Silently Disconnecting

**What goes wrong:**
The Hikvision alert stream (`/ISAPI/Event/notification/alertStream`) is a long-lived HTTP connection that delivers multipart XML events. It drops silently — the TCP connection closes without an error code, or the device goes offline, and the backend receives no notification. Attendance events stop arriving. No one notices until a supervisor asks why employees show zero hours.

**Why it happens:**
Developers treat the alert stream like a normal HTTP response — open, read once, close. The stream is actually persistent and device-dependent in its keep-alive behavior. Network interruptions, device reboots, and firmware updates all kill the connection without signaling the client. There are also per-device concurrent connection limits (often 4-8 active streams), so adding a second consumer silently drops one.

**How to avoid:**
- Implement a per-device connection supervisor: a Tokio task that owns the stream and detects EOF or timeout (heartbeat every 30s).
- On disconnect, log the event, emit a `DeviceStatus::Offline` event to the dashboard, and schedule exponential-backoff reconnection.
- Track the last-event-received timestamp per device; alert if no event in >5 minutes during business hours.
- Never share the alert stream connection — one consumer per device.

**Warning signs:**
- Dashboard shows device as "connected" but no recent events appear.
- `last_event_at` column on the devices table stops updating.
- Event stream connection count unexpectedly drops during high-load periods.

**Phase to address:** Device integration phase (webhook receiver + device manager).

---

### Pitfall 2: Hikvision XML Event Format Differs by Device Model and Firmware

**What goes wrong:**
The Hikvision ISAPI developer guide documents one event schema, but real devices emit variations. The `AccessControllerEvent` XML from a DS-K1T342 differs in field names and nesting from a DS-K1T341. Older firmware sends `employeeNo` as an integer; newer firmware sends `employeeNoString`. Some models omit `cardReaderNo`; others add non-standard fields. A rigid XML deserializer rejects valid events from one model while accepting another.

**Why it happens:**
Hikvision maintains parallel ISAPI guides for "Value Series" vs "Pro Series" face terminals (confirmed: separate 2024 guide published May 2024 vs 2022). Documentation is not cross-referenced. Developers test against one device model they own and assume the format is universal.

**How to avoid:**
- Use a forgiving XML parser with optional fields — never fail on unknown elements.
- Map both `employeeNo` (integer) and `employeeNoString` (string) → normalize to string in the event DTO.
- Log the raw XML of every unrecognized event type to a `device_events_raw` table for post-hoc analysis.
- During device commissioning, capture and store one sample event per device model in the database.
- Build an event-format abstraction layer: the domain layer never sees raw ISAPI XML — only normalized `AttendanceEvent` structs.

**Warning signs:**
- Parse errors in logs mentioning specific device IPs but not others.
- Employees on one reader registering attendance normally; employees on another showing gaps.
- `employeeNoString` field resolving to empty string (the integer-format device sent it as `employeeNo`).

**Phase to address:** Device integration phase. Test against all target device models before shipping v1.

---

### Pitfall 3: Storing Device Credentials in Plain Text

**What goes wrong:**
ISAPI credentials (username + password for each Hikvision device) are stored in the `devices` table as plain text. The database file is SQLite on a Windows/Linux on-premise machine. Any user with file system access — including the employee who manages the server — can read all device admin passwords. These same credentials typically control physical door access.

**Why it happens:**
It is the path of least resistance. Developers think "it's local, it's behind a firewall," but on-premise deployments are often on shared business servers. The database file may be inadvertently backed up to shared drives, sent to support, or exposed in a directory listing.

**How to avoid:**
- Encrypt device credentials at rest using a key derived from a server-specific secret (environment variable or OS keychain, never hardcoded).
- Use Rust's `secrecy` crate to prevent credential values from appearing in logs or debug output.
- Separate the devices table into `devices` (non-sensitive config) and `device_credentials` (encrypted secrets).
- Never log the password field — log only `device_id` and `username`.
- Document that the `.env` file containing the encryption key must not be committed to version control.

**Warning signs:**
- `SELECT password FROM devices` returns human-readable strings in the SQLite browser.
- Credentials appear in application logs during connection errors.
- Backup files contain the SQLite database without credential redaction.

**Phase to address:** Device manager phase (first time credentials are stored).

---

### Pitfall 4: Turso Offline Sync Conflict Resolution Is Not Implemented

**What goes wrong:**
Turso offline sync (public beta as of 2025) detects conflicts between local and remote writes but does **not** resolve them automatically. The official documentation states: "Conflict detection (but resolution is not yet implemented)" and explicitly warns "no durability guarantees, which means data loss is possible." If a supervisor edits a timesheet record locally while the cloud was offline, then the same record was administratively overridden in the cloud replica, one version silently wins based on last-push-wins — and the attendance audit trail may be corrupted.

**Why it happens:**
Developers assume "sync" means "eventually consistent without data loss." Turso's offline writes feature is designed for mobile apps with low-conflict workloads — not for attendance records where two admins at different sites might edit the same employee record during a network partition.

**How to avoid:**
- Treat the local SQLite as the **authoritative source** for all writes; cloud sync is backup + remote read, not a second write source.
- Never allow writes directly to the Turso cloud replica in the application logic — all writes go through the local instance first.
- Implement optimistic versioning: every mutable row has a `version` integer. On sync, compare versions; if remote version > local, flag for human review rather than silently overwriting.
- For the audit log specifically, make it append-only in both directions — conflicts in audit records mean duplication, not loss.
- Do not rely on Turso's beta conflict resolution becoming production-ready by your v1 launch date.

**Warning signs:**
- Timesheet records show unexpected changes after a network reconnection.
- Audit log entries are missing for changes that were confirmed in the UI.
- `sync()` calls succeed but the dashboard shows stale data.

**Phase to address:** Database/sync architecture phase (foundational — must be decided before any feature that writes to the DB).

---

### Pitfall 5: Time Calculations in Local Time Instead of UTC

**What goes wrong:**
Attendance records are stored with timestamps in local time. When DST changes occur (Mexico observes DST in some states; client business hours may span midnight), the math breaks:

- Spring forward: A worker who clocked in at 23:00 and out at 07:00 appears to have worked 7 hours instead of 8 (because 02:00 → 03:00 never existed).
- Fall back: A night shift worker appears to have worked 9 hours instead of 8 (02:00 occurs twice).
- Midnight shift: If shift boundary is 00:00 and the first punch is at 23:58 and last punch is 00:02 the next day, the day grouping is wrong.

**Why it happens:**
The Hikvision device sends timestamps in the format `2024-03-15T23:45:00+06:00` — the timezone offset is embedded. Developers truncate this to a naive datetime during parsing. All subsequent arithmetic is then on a local-time-anchored value that cannot distinguish the DST ambiguous hour.

**How to avoid:**
- Parse ISAPI timestamps as `DateTime<FixedOffset>` (chrono) and immediately convert to UTC for storage.
- Store all timestamps in the `attendance_events` table as UTC UNIX epoch (INTEGER) — never as local time strings.
- Convert to local time **only** in the presentation layer (frontend), using the IANA timezone configured for the installation.
- The shift window logic (first entry / last exit) must operate entirely in UTC — convert shift boundaries to UTC before comparing.
- Test explicitly with timestamps that cross the DST boundary for the target region.

**Warning signs:**
- Attendance reports show odd +/- 1 hour anomalies on DST change dates.
- Night shift workers show 7-hour or 9-hour days on specific dates in spring/fall.
- Datetime values in the DB have no timezone indicator (stored as `TEXT` without offset).

**Phase to address:** Time calculation engine phase — this must be correct from the first migration.

---

### Pitfall 6: Duplicate Event Processing from Multiple Readers

**What goes wrong:**
An employee badging at a door covered by two devices generates two ISAPI events within milliseconds. The system inserts two attendance records, and the "first entry / last exit" rule double-counts — or worse, the deduplication logic compares on `(employee_id, device_id)` instead of `(employee_id, timestamp_window)`, so both events survive as separate entries. The payroll export shows 16 work hours for a single 8-hour day.

**Why it happens:**
The first-entry/last-exit rule is designed for multi-device setups, but developers implement it as "group by device, take first/last per device," then aggregate — rather than "group by employee and time window across all devices." The multi-reader deduplication needs to be a session collapse algorithm, not a per-device grouping.

**How to avoid:**
- Define an idempotency window: events from the same `employee_id` within ±30 seconds are treated as one punch — store only one, discard duplicates.
- Implement deduplication at insert time using a database constraint: `UNIQUE(employee_id, ROUND(epoch_timestamp / 30))` or application-level check.
- The "first entry / last exit" aggregation must operate across all devices in the same shift window, treating the employee as a single stream of events.
- Log discarded duplicates to a `suppressed_events` table (not deleted) for auditability.

**Warning signs:**
- A single employee shows multiple entries within seconds on the raw events list.
- Daily work hours reports show values over 12h for standard employees.
- The events table grows faster than `employees * check-ins-per-day`.

**Phase to address:** Time calculation engine and webhook receiver phases.

---

### Pitfall 7: Audit Log That Is Only Application-Enforced, Not Database-Enforced

**What goes wrong:**
The audit trail is implemented as an application rule: "before every UPDATE to an attendance record, INSERT to audit_log." This works until:
- A developer runs a migration that updates records directly in SQLite without going through the application.
- The Turso sync overwrites a row during conflict resolution, bypassing the application layer.
- A future developer adds a bulk-edit feature and forgets to call the audit function.
- An admin accesses the SQLite file directly to "fix" a record.

There is now a gap in the audit trail. In legal disputes, a gap in the audit trail is as bad as having no audit trail.

**Why it happens:**
Application-layer audit enforcement feels "clean" in the code — it is in one place and easy to reason about. SQLite triggers feel like "database magic." Developers avoid triggers to keep logic in Rust.

**How to avoid:**
- Implement audit logging via SQLite triggers (`AFTER UPDATE`, `AFTER DELETE` on sensitive tables) as the **primary enforcement**, not the application layer. Application-layer calls are a secondary convenience.
- Store triggers in the migration files so they are version-controlled and cannot be silently dropped.
- Make the `audit_log` table INSERT-only via a database-level CHECK constraint: no `rowid` can be deleted or updated (enforce with trigger).
- Add a hash chain: each audit log entry stores `SHA256(previous_row_hash || this_row_data)`. A break in the chain is detectable.
- For legal compliance, consider periodically writing the chain root to an external immutable store (even a simple email to the business owner counts as an anchoring mechanism).

**Warning signs:**
- Audit log shows the previous value as NULL for some updates (means trigger fired late or not at all).
- The audit table has fewer rows than the number of recorded changes in the application log.
- `DELETE FROM audit_log WHERE ...` succeeds without error (means no guard in place).

**Phase to address:** Database schema phase and audit trail feature phase.

---

### Pitfall 8: Facial Enrollment Failure Modes — Backlighting and Pose Variation

**What goes wrong:**
Employees enroll their face in the office during setup. Later, the device at the factory entrance fails to recognize them because lighting conditions differ (backlight from a window behind the employee, harsh fluorescent overhead lighting). The system either rejects valid employees (false negatives triggering manual punch-in workarounds) or, worse, accepts a photo printout as a valid biometric (presentation attack).

**Why it happens:**
Enrollment is done once in ideal conditions. Operators do not understand that facial recognition template quality degrades when the enrollment photo's lighting does not match recognition conditions. The Hikvision device's built-in "enrollment mode" captures one frontal photo — no multi-angle, no lighting-variation samples.

**How to avoid:**
- During enrollment, capture the photo using the actual device camera at the deployment location — not a webcam in the office or a JPG upload — so the template matches real-world conditions.
- Enforce minimum face quality score before accepting enrollment (Hikvision devices expose a quality threshold in ISAPI config).
- Document lighting requirements in the installation guide: no strong light source behind the employee.
- For JPG upload fallback (manual enrollment), warn the operator if face quality score < threshold.
- After enrollment, run a test recognition pass and display the confidence score to the operator.
- Offer re-enrollment without deleting the old template until the new one is confirmed working.

**Warning signs:**
- False rejection rate > 5% at a specific device but not others (device has different lighting conditions).
- Employees using the "manual punch" workaround consistently at one entrance but not another.
- Enrollment photos show strong shadow or backlit faces.

**Phase to address:** Facial enrollment modal phase and device commissioning checklist.

---

### Pitfall 9: Midnight Shift Attribution — Wrong Day Assignment

**What goes wrong:**
An employee works 22:00–06:00. Their last exit event at 06:00 on day D+1 is attributed to day D+1 in the daily summary. The system reports 8 hours on D+1 and 0 hours on D — instead of 8 hours on D. The employee's D payroll day shows as absent. When the supervisor corrects it manually, the audit trail shows a suspicious edit that looks like timesheet fraud.

**Why it happens:**
The shift window is defined as "group events by calendar date of the event timestamp." Overnight shifts naturally span two calendar dates. Without an explicit shift-definition concept (start time + duration that can exceed midnight), the grouping is wrong by design.

**How to avoid:**
- Define shifts as `(anchor_date, start_time, max_duration)` — a shift starting 22:00 with max_duration 10h owns all events from 22:00 to 08:00, regardless of which calendar date they fall on.
- The anchor date is the date of the **first entry** (not the last exit) for that shift window.
- Configuring shift boundaries per department allows the engine to correctly attribute late-night punches to the previous day's shift.
- The tolerance window (±20 min) must be applied relative to the shift start/end, not to calendar midnight.

**Warning signs:**
- Night shift workers consistently show 0 hours on their shift start day.
- Overtime hours appear on the wrong payroll period for overnight workers.
- Manual adjustments cluster around night shift employees in the audit trail.

**Phase to address:** Time calculation engine phase.

---

### Pitfall 10: Security — Default Admin Password on Hikvision Devices

**What goes wrong:**
Hikvision devices ship with default credentials (`admin`/`12345` or similar). On-premise deployments on the same LAN as the business mean any employee with network access can access the device web UI, download stored face templates, or disable attendance tracking. Face template data is biometric PII — its exposure is a legal liability in jurisdictions with biometric privacy laws (GDPR Article 9, Mexico's LFPDPPP).

**Why it happens:**
IT staff change the device password during initial setup, but the Cronometrix application also needs to store that credential. When the IT person changes the password in the device but forgets to update the application config, the integration breaks — so there is pressure to use simple, never-changing passwords. Default passwords are the path of least resistance.

**How to avoid:**
- During device commissioning, the application should verify the device is **not** using a known default password and reject commissioning if it is.
- Implement a "test connection" flow that validates credentials immediately and shows which devices have default credentials as a dashboard warning.
- Store the credential hash in the device record. If the hash matches a list of known defaults, emit a persistent security alert.
- Document network segmentation: biometric devices should be on a VLAN that only the Cronometrix server can reach.

**Warning signs:**
- Device web UI is accessible from employee workstations on the same subnet.
- Credentials in the Cronometrix device table match `admin/12345` or `admin/admin`.
- No network segmentation between access control devices and general office network.

**Phase to address:** Device manager and security hardening phase.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Store timestamps as local-time TEXT strings | Simpler initial query | DST bugs, migration nightmare when changing regions | Never |
| Application-only audit enforcement (no triggers) | Cleaner Rust code | Audit gaps from direct DB access, migrations, sync conflicts | Never for legal trail |
| Poll `/ISAPI/AccessControl/AcsEvent` instead of alert stream | Simpler to implement | Misses events during polling gaps, can't guarantee real-time | Only for dev/demo |
| Hard-code XML field names without fallback | Fast first implementation | Breaks on any device model variation | Only in unit tests with known fixture |
| Turso sync with no versioning | Zero extra DB columns | Silent data loss on conflict | Never for attendance records |
| Skip deduplication window | Simpler insert path | Double-counted hours on multi-reader setups | Never in production |
| Embed device credentials unencrypted in app config | Easy dev setup | Credential exposure via config file leaks | Only in local dev (`.env` not committed) |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Hikvision alert stream | Single HTTP GET treated as one-shot response | Long-lived persistent connection with supervisor task and reconnect logic |
| Hikvision ISAPI auth | Using Basic Auth (works on some firmware, fails on others) | Use Digest Auth by default; detect 401 WWW-Authenticate header to negotiate |
| Hikvision event XML | Strict deserialization that fails on unknown fields | Permissive parser; map known fields, log unknown fields, never reject on extras |
| Turso embedded replica | Opening local DB while sync is running | Use Turso's sync-aware checkpoint; never open local file concurrently with sync |
| Hikvision face enrollment | Uploading JPG from arbitrary source | Use device camera at installation site; validate quality score before committing |
| Hikvision webhook push (device-initiated) | No authentication on the receive endpoint | Validate `X-Hikvision-Signature` header or configure Basic Auth on listener |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Calculating daily summaries on every dashboard load | Dashboard latency climbs linearly with attendance event volume | Materialize daily summaries via background job; query pre-aggregated table | ~500 employees, 30 days history |
| Querying `attendance_events` without index on `(employee_id, timestamp)` | Timesheet load is slow; full table scans in SQLite explain output | Add composite index at migration; add `EXPLAIN QUERY PLAN` check in CI | ~50,000 events in table |
| Fanout webhook delivery: sync call to 4 devices per enrollment | Enrollment modal hangs for 10–30s; UI blocks | Spawn concurrent tokio tasks for per-device enrollment; return optimistic success with per-device status | First device that is slow |
| Loading all audit log rows to paginate in application code | Audit trail page load is slow and memory-heavy | Use OFFSET/LIMIT at SQL level; add `created_at` index on audit_log | ~10,000 audit entries |
| Re-parsing XML for each event to extract device serial | CPU usage spikes during high-traffic periods | Parse device identity once at connection establishment; attach to all events from that stream | Multiple devices + high badge frequency |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Device ISAPI credentials stored as plain text in SQLite | Any DB file leak exposes credentials for physical door control | Encrypt at rest using server-derived key; use `secrecy` crate in Rust |
| No authentication on the Cronometrix webhook receive endpoint | Any device on the LAN can inject fake attendance events | Validate request comes from configured device IP; optionally validate HMAC/Basic Auth |
| Biometric face templates synchronized to cloud without encryption | Biometric PII in cloud replica constitutes highest-risk data category | Encrypt face template blobs before inserting; key management separate from data |
| Admin role with no session expiry | Long-lived sessions allow privilege abuse | Implement session timeout; require re-auth for destructive operations (deletion, bulk edits) |
| Audit log visible to all roles | Supervisors can see who edited what about them | Restrict audit log read access to Admin role only |
| SQLite database file in web-accessible directory | DB file downloadable via HTTP | Database file must be outside web root; Axum serves only API responses, never static files from DB path |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Showing raw UTC timestamps to admins | Admins cannot reconcile with employee's reported clock-in time | Always display timestamps in installation's configured timezone |
| Audit trail shows `old_value: null` for all changes | Audit is useless for investigation — no before-state | Capture full row snapshot as `old_value` JSON at time of change trigger |
| Enrollment modal closes immediately without showing per-device result | Admin doesn't know which devices succeeded and which failed | Show per-device enrollment status (syncing... / enrolled / failed) before allowing close |
| Timesheet corrections require justification but the field is optional in the UI | Legal audit requirement bypassed by accident | Make justification a required field with minimum length; block submit until populated |
| "Device offline" shown only in device manager page, not dashboard | Admins don't notice a reader is down until employees report missed punches | Surface device health as a prominent banner/alert on the main dashboard |

---

## "Looks Done But Isn't" Checklist

- [ ] **Alert stream receiver:** Verify reconnection logic fires after device reboot — test by rebooting a physical device mid-shift.
- [ ] **Time calculations:** Verify overnight shift produces correct hours when tested with timestamps that cross midnight.
- [ ] **DST edge case:** Verify hours calculation is correct for the DST spring-forward night and the fall-back night for the target region.
- [ ] **Duplicate events:** Verify that badging the same card twice within 10 seconds creates only one attendance event.
- [ ] **Audit trail triggers:** Verify `UPDATE attendance_records SET ... WHERE ...` executed directly in the SQLite CLI still produces an audit log entry.
- [ ] **Conflict simulation:** Verify what happens when the Turso sync is run after offline edits — are records preserved or silently overwritten?
- [ ] **Enrollment quality:** Verify the enrollment flow rejects a low-quality image (dark photo, face too small, extreme angle).
- [ ] **Credential encryption:** Verify `SELECT password FROM device_credentials` in SQLite browser returns ciphertext, not plain text.
- [ ] **RBAC enforcement:** Verify a Supervisor-role token cannot access the audit trail API or modify global configuration.
- [ ] **Event format variants:** Verify the XML parser successfully processes a sample event from each target device model (DS-K1T341, DS-K1T342, DS-K1T604 if used).

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Missed events due to silent stream disconnect | MEDIUM | Backfill from device's local event log via `/ISAPI/AccessControl/AcsEvent` query endpoint; mark as "backfilled" |
| Wrong timestamps due to local-time storage | HIGH | Migration to recalculate UTC from known timezone offset; requires knowing original timezone of each event |
| Corrupted audit trail due to bypass | HIGH | Restore from last known-good Turso cloud snapshot; diff against local for gap period |
| Conflict data loss on Turso sync | HIGH | Restore from last cloud checkpoint; replay local WAL changes manually |
| Face template enrollment quality failure | LOW | Re-enroll at deployment site with device camera; old template remains active during transition |
| Duplicate events already in production | MEDIUM | Write deduplication migration: identify and soft-delete duplicates within 30s windows; mark originals as authoritative |
| Device credentials exposed | HIGH | Rotate all device credentials immediately; audit device access logs for unauthorized commands |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Silent alert stream disconnect | Device integration (webhook receiver) | Reboot test: device offline → reconnect within 60s |
| XML format variation by device model | Device integration + QA | Parse test fixtures from all target device models |
| Plain text credential storage | Device manager (first credential storage) | SQLite CLI check returns ciphertext |
| Turso sync conflict resolution absent | Database/architecture phase (foundational) | Document explicit "local is authoritative" decision; no direct cloud writes |
| UTC time storage (DST) | Time calculation engine phase | DST boundary test — overnight shift on change date |
| Duplicate event deduplication | Webhook receiver + time engine | Two events within 30s → one record in attendance_events |
| Application-only audit enforcement | DB schema phase + audit trail feature | SQLite CLI UPDATE → audit_log row appears |
| Facial enrollment quality | Enrollment modal phase | Reject dark/blurry photo; show quality score |
| Midnight shift wrong-day attribution | Time calculation engine phase | Night shift: last exit at 06:00 attributed to prior-day shift |
| Default device passwords | Device manager + commissioning | Commissioning rejects device with known default credential |
| Webhook receive endpoint security | Device integration phase | Curl from unknown IP is rejected |

---

## Sources

- Hikvision ISAPI configuration via API (dev.to/gordinmitya): practical gotchas around XML templates, model differences
- HikVision-EventReceiver (github.com/peku33): alert stream connection handling and multipart XML parsing patterns
- hikvision-isapi Laravel package (github.com/Shaykhnazar): event schema reference, webhook auth options, host limits
- Turso Offline Sync Public Beta announcement (turso.tech/blog): conflict resolution not implemented, durability caveat
- Turso embedded replicas docs: concurrent open warning, WAL management, last-push-wins behavior
- Five Common DST Antipatterns (codeofmatt.com): overnight shift payroll calculation impact
- Biometric enrollment best practices (h33.ai/blog, facia.ai): lighting failure modes, quality thresholds, FTE/FTC
- Immutable audit trail design (designgurus.io, mattermost.com/blog): application-layer bypass risk, hash chain approach
- Hikvision DS-K1T342 user manual and ISAPI guide Value Series 2024 (scribd.com): field name variants
- Hikvision ISAPI general application PDF (github.com/loozhengyuan): alert stream endpoint and authentication modes

---
*Pitfalls research for: Biometric time & attendance system (Cronometrix)*
*Researched: 2026-04-11*
