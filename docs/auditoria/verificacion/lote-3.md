# Verification — M-01 to M-09 (Medios), against current worktree HEAD

Verified read-only in `/home/gerswin/Proyectos/cronometrix/.claude/worktrees/scratch`
(branch `chore/trim-claude-md`, working tree unmodified). Line numbers below are
from the current tree, not the audited `9b36341`.

---

## M-01 — Deduplicación insuficiente entre dispositivos y para desconocidos

**Verdict: NUANCED** — substance real, but the audit's framing would mislead an
implementer given the design decided after the audit.

**Evidence:**
- `backend/src/db/migrations/004_attendance_events.sql:2,22-23` — the unique
  dedup index is `(employee_id, device_id, direction, bucket_30s)`. Comment at
  the top of the file states explicitly: *"SQLite treats NULL != NULL in
  UNIQUE, so unknown-face rows (employee_id IS NULL) intentionally all
  persist. This matches D-07's forensic intent."* So both halves of M-01's
  claim are literally true today: (a) `device_id` is part of the key, so the
  same physical mark registered on two different readers is never
  deduplicated, and (b) unknown-face events never dedup by construction.
- `backend/src/calc/aggregation.rs:88-92` — aggregation picks
  `entries.first()` / `exits.last()` as canonical, i.e. only the two extreme
  timestamps matter for hours math. A same-employee duplicate at a second
  device therefore does **not** inflate `work_minutes` unless it shifts one
  of the two extremes — the audit's "agregación errónea" risk is real but
  narrower than "duplicate rows corrupt totals" (it's really about which
  timestamp becomes canonical — a replay/fraud vector — not row-count
  inflation).
- I could not find a test that "expressly accepts" cross-device duplication
  (searched `event_tests.rs`, `listener_tests.rs`, `device_push_test.rs`); the
  tests I found (`second_identical_event_deduplicates`,
  `duplicate_queued_event_leaves_no_second_photo`) both replay against the
  **same** `device_id`. That specific audit sub-claim did not check out.
- `backend/src/db/migrations/027_device_push_inbox.sql:1-12` (post-audit) —
  the new durable push inbox is a **separate, earlier** layer than
  `attendance_events` and deliberately does not dedup at all: *"dos cuerpos
  idénticos pueden ser dos eventos legítimos... un falso positivo de
  deduplicación perdería una marcación real... La deduplicación ya vive en
  bucket_30s de attendance_events."* This is not what M-01 cites (M-01 cites
  `004_attendance_events.sql`, the structured table, not the inbox), but it
  is highly relevant context: the team has already weighed "dedup harder" vs.
  "risk losing real events" and chose to keep dedup narrow and DB-invariant
  rather than fuzzy.

**The trap for an implementer:** M-01's recommendation — "identificador
nativo del evento o hash canónico" for cross-device/cross-unknown dedup — is
exactly the kind of broadened, content-based dedup the team explicitly
rejected at the inbox layer for fear of dropping legitimate concurrent
captures (e.g., two doors at the same physical entrance, or near-simultaneous
retries). Implementing it naively at the `attendance_events` layer risks the
same failure mode: collapsing two genuinely distinct captures into one and
silently losing a real attendance mark. If this is actioned, it should be a
**ledger/flag of likely duplicates without removing or merging rows** (which
the recommendation does also say — "sin borrar evidencia" — so the
recommendation itself is not wrong, just easy to over-implement past that
qualifier).

---

## M-02 — Evento desconocido contamina a todos los empleados

**Verdict: CONFIRMED**

**Evidence:**
- `backend/src/daily_records/service.rs:123-129`:
  ```sql
  SELECT id, employee_id, device_id, direction, captured_at, is_unknown
  FROM attendance_events
  WHERE (employee_id = ?1 OR (employee_id IS NULL AND is_unknown = 1))
    AND captured_at BETWEEN ?2 AND ?3
  ```
  Every unknown-face event in the employee's shift window is pulled in
  regardless of `device_id` — there is no join/filter tying the unknown event
  to a device the employee could plausibly have used.
- `backend/src/calc/aggregation.rs:73-80` sets `unknown_in_window = true` for
  any such row, excluding it from anchoring but keeping the flag.
- `backend/src/calc/engine.rs:53-55`: `if agg.unknown_in_window { anomalies.push(AnomalyCode::UnknownFaceInWindow) }`.

Net effect: one unassociated face capture at any device, at any time inside
any employee's shift window that day, raises `UNKNOWN_FACE_IN_WINDOW` for
every employee whose window overlaps that timestamp — exactly the audit's
claim. Still unfixed.

---

## M-03 — Primera entrada/última salida y almuerzo fijo inflan o reducen jornada

**Verdict: CONFIRMED**

**Evidence:**
- `backend/src/calc/aggregation.rs:88-92` — `canonical_entry = entries.first()`
  (earliest), `canonical_exit = exits.last()` (latest); intermediate
  entry/exit pairs (e.g., a personal errand mid-shift) are invisible to the
  hours calculation except via the lunch-pairing logic.
- `backend/src/calc/lunch.rs:20-21` — `"fixed" => (fallback, None)`: in fixed
  mode the nominal lunch duration is deducted **unconditionally**, with no
  check for whether the shift was even long enough to contain a lunch, or
  whether any break was actually taken.
- `backend/src/calc/lunch.rs:33-48` — in punch mode, only the **first**
  mid-shift `(exit, entry)` pair is used (`.find(...)` returns the first
  match); a second break in the same day is silently ignored and counted as
  work.
- `backend/src/calc/engine.rs:75`: `let work = (raw_minutes - lunch_ded).max(0);`
  — confirms "jornadas cortas pueden quedar infravaloradas": a short shift
  with a fixed lunch larger than the elapsed time clamps to zero rather than
  reporting a negative/partial value operators could investigate.

All three sub-claims (extremes-only anchoring, unconditional fixed-lunch
deduction, first-pair-only punch matching) check out against current code.

---

## M-04 — Permisos solo de día completo y filtros incoherentes

**Verdict: CONFIRMED**

**Evidence:**
- `backend/src/calc/engine.rs:18-38` — when `input.leave` is `Some(..)`, the
  engine unconditionally returns `work_minutes: 0, overtime_minutes: 0,
  late_minutes: 0, early_departure_minutes: 0` for the whole day; there is no
  partial/hourly leave path anywhere in `EngineInput`/`DailyRecordOutput`.
- `shift_type` filter asymmetry, still present: `backend/src/reports/service.rs:261-263`
  applies `dr.shift_type = ?` to the main daily_records-joined query, but the
  secondary leave-only aggregation query built at
  `backend/src/reports/service.rs:618-670` (`leave_predicates`/`leave_sql`)
  has no `shift_type` predicate at all — confirmed by reading the full
  predicate list, which only contains status/deleted_at/date-range/
  employment-window/department/employee filters.
- Calendar vs. business-day counters mixed in the same report:
  `backend/src/reports/service.rs:753-758` computes `días_ausentes` as Mon-Fri
  only (`.filter(|d| d.weekday().num_days_from_monday() < 5)`), while the
  leave-day accumulator at `backend/src/reports/service.rs:738-741` inserts
  **every** calendar day in the leave's overlap with the period into
  `leave_dates` with no weekday filter. So in one report, "días ausentes" is
  business-day-only and "días de permiso/vacación" is calendar-day-inclusive
  — the audit's "mezclan días calendario y laborales" claim is accurate.

---

## M-05 — Lecturas de reporte sin snapshot consistente

**Verdict: CONFIRMED**

**Evidence:**
- `backend/src/reports/service.rs:98-869` (`compute_report` /
  `record_export`) opens one connection (`conn = ... .connect()...` at line
  ~131) and issues multiple independent `conn.query(...)` calls in sequence:
  the main daily_records-joined aggregation, then a second, separate query
  against `leaves` (line ~618 onward), plus whatever employee/department
  lookups precede them. I grepped the whole file for `BEGIN`,
  `transaction`, `snapshot`, `isolation` — none appear. Each `query()` call is
  its own implicit autocommit statement; nothing pins a single MVCC/WAL
  snapshot across the sequence.
- Consequence: a write landing between the daily_records query and the leaves
  query (e.g., an override approval, or a leave being cancelled) is visible
  to one half of the report and not the other, producing an internally
  inconsistent report — exactly the audit's claim.

---

## M-06 — Auditoría inmutable pero incompleta y sin evidencia antimanipulación fuerte

**Verdict: CONFIRMED**

**Evidence:**
- Immutability control is real: `backend/src/db/migrations/020_audit_immutability.sql:154-164`
  defines `audit_log_immutable_update`/`_delete` triggers that
  `RAISE(ABORT, 'audit_log is immutable')` on any UPDATE/DELETE — this part
  of the audit's own description is a fair, positive control, correctly
  reported as such.
- Actor-null triggers, still present: `backend/src/db/migrations/017_phase7_audit_triggers.sql:9-10,21-31`
  — enrollment/face-mapping triggers write `actor_id = NULL` by design
  ("the application already wrote started_by/created_by so actor surfaces
  via those columns on a JOIN"). This is a documented tradeoff, not an
  oversight, but it does mean `audit_log.actor_id` alone cannot answer "who
  did this" without a secondary join/parse — matching the audit's concern,
  just less alarmingly than "actor is lost."
- Employee audit trigger does not track later-added columns, confirmed
  still true: the **last** redefinition of `audit_employees_insert/update/delete`
  is `backend/src/db/migrations/018_employees_base_salary.sql:15-71`, whose
  JSON snapshot only carries `id, employee_code, name, department_id, status,
  base_salary_cents, version`. Columns added to `employees` before 018
  (`position`, `hire_date` in `015_employees_position_hire_date.sql`,
  `face_id`/`current_face_enrollment_id` in `016_enrollments.sql`) and after
  018 (`salary_kind` in `024_employee_salary_kind.sql`, `terminated_on` in
  `026_employee_terminated_on.sql`) are absent from every audit row for
  employees, and no later migration re-touches the trigger (`grep -l
  audit_employees` only matches `002`, `014`, `018`). A salary-kind or
  hire-date change on an employee today leaves **no** audit trail field for
  that value.
- No hash chain / external anchor: `grep -rl "hash_chain\|prev_hash\|merkle"
  backend/src` returns nothing. The recommendation's hash-chaining/WORM
  export gap is real and unaddressed.

---

## M-07 — Evidencia de permisos confía en `Content-Type`

**Verdict: CONFIRMED**

**Evidence:**
- `backend/src/leaves/handlers.rs:104-118` — the evidence branch reads
  `field.content_type()` (client-supplied multipart header) and switches on
  the literal string (`"application/pdf" => Some("pdf")`, etc.) with no
  inspection of the actual bytes.
- `backend/src/daily_records/handlers.rs:44-58,120-151` — by contrast, this
  handler explicitly derives the extension from magic bytes
  (`infer_evidence_ext_from_magic`), with a doc comment: *"CR-03 mitigation:
  derive evidence file extension from magic bytes rather than the
  client-supplied multipart Content-Type."*

The asymmetry the audit describes is real and unaddressed: the fix applied
to override evidence (CR-03) was never carried over to leave evidence.

---

## M-08 — Calidad facial se confía al navegador y no hay PAD/liveness

**Verdict: CONFIRMED**

**Evidence:**
- `backend/src/enrollments/models.rs:99-104` — doc comment states outright:
  *"Trust boundary: the backend does not run a second face detector. It
  rejects malformed or internally inconsistent client evidence, enforces the
  same small acceptance thresholds published by
  frontend/src/lib/face-detection.ts, and separately decodes/normalizes the
  submitted JPEG before persistence."*
- `backend/src/enrollments/models.rs:128-160` (`FaceQualityEvidence::validate`)
  only checks internal consistency of client-reported numbers (finite,
  in-range, `luminanceOk`/`sizeOk` not contradicting the raw numbers) and
  requires all three client-reported booleans to be true. It never
  re-derives `face_detected`/`luminance`/dimensions from the actual JPEG
  bytes, and there is no liveness/presentation-attack-detection step
  anywhere in the enrollment path.

This is a self-documented, known limitation, not a hidden bug — but the
audit's description is accurate and the gap is real.

---

## M-09 — Bearer en query SSE y cabeceras web defensivas ausentes

**Verdict: CONFIRMED, with one already-mitigated sub-claim**

**Evidence:**
- SSE bearer-in-query, still present by design:
  `backend/src/main.rs:259-261` — comment: *"SSE stream: EventSource cannot
  send Bearer headers (T-4-02), so auth is handled inside the handler via
  ?token=<jwt> query param."* `backend/src/events/handlers.rs:27,32,40` route
  the token through `?token=<access_jwt>` and verify it as a normal access
  JWT, with a comment calling this an "accepted risk on-premise."
- Direct request-log leakage is partially mitigated (post-audit change,
  matching the task's briefing): `deploy/nginx.conf:44-49` sets
  `access_log off` and `error_log /dev/stderr crit` for
  `/api/v1/events/stream`, and a parallel comment block at lines 62-69
  confirms the same treatment was extended to the device-push token route.
  This does **not** close the audit's actual concern, though — it only
  stops nginx's own logs from recording the token. Browser history, any
  intermediate proxy not covered by this config, and Referer headers from
  the SSE page can still carry the token in the URL, which is the audit's
  literal claim ("historial/proxies pueden conservarlo") and remains true.
- Missing headers, confirmed: full read of `deploy/nginx.conf` (102 lines)
  shows no `add_header` for HSTS, CSP, `X-Content-Type-Options: nosniff`,
  `X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy`, or
  `Cache-Control: no-store` anywhere in the file — the only `add_header` is
  `Content-Type: text/plain` on the `/gateway-health` probe.

---

## Summary of surprises

- M-01 is the one in this batch that most needs a rewrite before acting on
  it: its own cited evidence (`004_attendance_events.sql`) is accurate, but
  its recommendation, if applied broadly, runs straight into a design
  decision (`027_device_push_inbox.sql`) the team made specifically to avoid
  losing real events to false-positive dedup — the same trap the task
  description warned about existing elsewhere in the audit.
- The "test explicitly accepts the cross-device case" evidentiary detail in
  M-01 did not check out on inspection — the two dedup tests I found both
  replay against the same device, not different devices.
- M-06's actor-null concern is real but softer than the audit implies for
  the enrollment tables specifically — it's a documented, deliberate
  design (actor recoverable via JOIN on `started_by`/`created_by`), not a
  silent loss — while the missing-employee-fields half of M-06 is a clean,
  unambiguous gap with no mitigating design note at all.
