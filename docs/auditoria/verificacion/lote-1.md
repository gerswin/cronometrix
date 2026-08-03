# Verification — H-02, H-05, H-06, H-07

Audited commit: `9b36341`. Verified against current worktree HEAD
(`48af434`, branch `docs/verificacion-lote2`) in
`/home/gerswin/Proyectos/cronometrix/.claude/worktrees/scratch`.

---

## H-02 — Salida de turno nocturno se atribuye al día equivocado

**Verdict: CONFIRMED**

### Evidence

`backend/src/events/service.rs:164-174` (was 165-173 at audit time — one-line
drift only):

```rust
let recompute_request = event.employee_id.as_ref().and_then(|employee_id| {
    Utc.timestamp_opt(event.captured_at, 0)
        .single()
        .map(|captured_at| RecomputeRequest::Day {
            employee_id: employee_id.clone(),
            anchor_date: captured_at
                .with_timezone(&state.config.timezone)
                .date_naive(),
        })
});
```

The `anchor_date` sent to the recompute worker is the **local calendar date
of the event itself**, not the shift's start day. For an overnight shift
(e.g. entry 22:00 Monday → exit 06:00 Tuesday):

- The entry event (local date = Monday) correctly triggers
  `RecomputeRequest::Day { anchor_date: Monday }`.
- The exit event (local date = Tuesday) triggers
  `RecomputeRequest::Day { anchor_date: Tuesday }` — the wrong day.

`backend/src/recompute/worker.rs:110-149` (`process_day`) calls
`dr_service::recompute_for_day(state, employee_id, anchor_date)` verbatim
with whatever `anchor_date` it was handed — there is no shift-aware
resolution step anywhere in the ingestion→recompute pipeline that maps a
post-midnight capture back to the shift's start day.

`backend/src/daily_records/service.rs:39-121` (`recompute_for_day`) *is*
correct **once given the right anchor_date**: it delegates window
construction to `shift_window_overnight_aware`, which for
`is_overnight_shift = true` extends `window_end` to `anchor_date + 1`, so the
`captured_at BETWEEN window_start AND window_end` query at lines 123-129
picks up the post-midnight exit — this is exactly what the audit's citation
of `daily_records/service.rs:110-129` ("solo cruza medianoche si recibe el
ancla correcta") means, and it checks out.

Confirmed there is no compensating mechanism: I searched the ingestion
path (`backend/src/events/`, `backend/src/isapi/`, `backend/src/devices/`,
`backend/src/recompute/`) for any logic that also enqueues `anchor_date - 1`
for overnight departments, or that resolves "which shift does this event
belong to" — none exists.

An existing integration test,
`backend/tests/daily_record_tests.rs::recompute_overnight_captures_post_midnight_events`,
proves the SQL/window logic is correct — but it calls
`dr_service::recompute_for_day(&state, &emp_id, anchor)` **directly with the
correct Monday anchor**, bypassing the event→anchor_date derivation entirely.
It does not exercise the actual bug. A second test,
`backend/tests/event_tests.rs:404-415`, actually documents the buggy
derivation as intended behavior: it asserts a `captured_at` of epoch
`1_700_000_000` (2023-11-14 UTC) produces
`RecomputeRequest::Day { anchor_date: 2023-11-14, .. }` — i.e. literally
"the recompute request uses the event's own local date," which is the root
cause the audit names.

### Consequence if "fixed" naively

The audit's evidence and risk description are accurate as written; no trap
here. The fix is *not* to change `daily_records/service.rs` (already
correct) — it is to make `events/service.rs`'s recompute-request
construction shift-aware: look up the employee's department
`is_overnight_shift`/`shift_start_time` before deriving `anchor_date`, so an
exit event whose local clock time falls before `shift_start_time` resolves
to `local_date - 1`, not `local_date`. The audit's own recommendation
("resolver candidatos de turno por ventana... idempotente") points the same
way.

---

## H-05 — Horas extra legales incompletas y tope diario mal calculado

**Verdict: NUANCED (headline claim is now FALSE / ALREADY FIXED; the
broader finding is still CONFIRMED)**

### What's already fixed

`backend/src/calc/overtime.rs:16-28` (current):

```rust
// LOTTT 178: el tope es de 10 h EFECTIVAS al día. `work_minutes` ya incluye
// los extraordinarios (calc/engine.rs:82); sumarlos otra vez evalúa una
// jornada que nadie trabajó.
if work_minutes > 600 {
    out.push(AnomalyCode::OtCapExceededDaily);
}
```

This is exactly the fix the audit recommended ("corregir el contador
diario"). The double-counting bug the audit describes
(`work_minutes + overtime_minutes > 600`, since `work_minutes` already
contains the overtime slice) is gone. Confirmed via git history:

```
e8b04c5 fix(reports): pay overtime once at 150% and stop double-charging lateness (C-01, C-02)
```

`git merge-base --is-ancestor 9b36341 e8b04c5` succeeds (fix postdates the
audited commit) and `e8b04c5` is an ancestor of current HEAD. The diff at
that commit shows precisely `- if work_minutes + overtime_minutes > 600` →
`+ if work_minutes > 600`.

The test the audit cited as "codifying the wrong sum"
(`overtime.rs:39-45` at audit time) is now
`daily_cap_triggers_only_when_the_real_workday_exceeds_600` (lines 42-47)
and asserts the *correct* behavior — it no longer documents a bug.

### What's still real

The audit's second half stands: the code raises `OtCapExceededDaily` /
`Weekly` / `Annual` as advisory anomalies only (`calc/anomalies.rs:12-14`,
surfaced via the supervisor review queue) — it never blocks the excess,
never requires or records authorization/causal justification, and produces
no LOTTT Art. 178/182 "urgent exception" pathway or exportable/immutable
overtime register. I grepped the whole backend for any
authorization/consent concept tied to overtime
(`overtime.*auth`, `ot_authoriz`, etc.) — none exists.

### If you were to act on this

Do **not** re-touch the daily-cap arithmetic — that part is done and
correct. The remaining work is entirely the governance layer described in
the audit's recommendation: mandatory authorization + causal capture before
persisting an OT-triggering record, and an exportable/immutable log. Anyone
implementing "H-05" from the informe text alone, without re-reading the
current code, would likely re-break the already-fixed daily cap by
"fixing" it back toward `work_minutes + overtime_minutes`, since the
informe's evidence quote still shows the old formula. Worth flagging to
whoever re-reads this finding.

---

## H-06 — Vacaciones y bono vacacional sin remuneración completa

**Verdict: NUANCED (headline overstates the gap; a narrower, self-documented
edge case is confirmed; the LOTTT seniority/anniversary claim is confirmed
and out of my scope to adjudicate legally)**

### What the audit's own citation actually says

The audit cites `reports/service.rs:359-375`, described as "cuenta días de
vacaciones que solo existen como permisos, pero no crea pago." I diffed
that exact line range at commit `9b36341` — it is the **W-5 secondary
aggregation comment block**, not the vacation-pay code path. Verbatim (still
present today at `backend/src/reports/service.rs:612-618`):

```rust
// Money treatment for leaves WITHOUT a daily_record overlay is a known v1
// limitation: only overlays attached to a daily_record produce vacation
// pay (the daily_records branch above). Leave-only days produce counter
// increments and entry into leave_dates (so absent-day calc skips them)
// but no pay is synthesized. Future work could synthesize vacation pay
// for leave-only days; out of scope for v1.
```

### Why the headline is misleading

There **is** a vacation-pay code path, and it already existed at the
audited commit (`git show 9b36341:.../service.rs` confirms it, unchanged
in substance from today) — `backend/src/reports/service.rs:417-447`:

```rust
Some("vacation") => {
    entry.leave_dates.insert(anchor_date);
    match require_salary_kind(salary_kind_str_opt.as_deref()) {
        Some(salary_kind) => {
            let work_pay = money::work_pay_cents(
                ordinary_daily_minutes, base_salary_cents,
                ordinary_daily_minutes, salary_kind,
            );
            entry.agg.work_pay_cents = entry.agg.work_pay_cents.saturating_add(work_pay);
            entry.agg.total_a_pagar_cents = entry.agg.total_a_pagar_cents.saturating_add(work_pay);
        }
        None => { /* SALARY_KIND_MISSING anomaly, no invented amount */ }
    }
}
```

And crucially, vacation leaves reach this path automatically in the normal
case: `backend/src/leaves/service.rs:96-160` (create) and `:309-356`
(similar path) enqueue `RecomputeRequest::Range` over the leave's date span
on creation, and `daily_records/service.rs:39-121`
(`recompute_for_day`) unconditionally upserts a `daily_records` row —
including the `leave_id` overlay — for every day in that range, even with
zero attendance events. So a vacation leave created through the normal
service path *does* get a `daily_records` row with the overlay attached,
and *is* paid at the ordinary daily rate. The "known v1 limitation" the
comment documents is a real but narrower gap: days whose `daily_records`
row for some reason never got the overlay attached (e.g. recompute worker
unavailable at the time, employee inactive when recompute ran, or a leave
row that predates/bypasses the recompute-on-create trigger — I did not find
such a path in the current codebase, but the comment's own framing implies
the authors were guarding against exactly this). In that edge case, and
only that edge case, the day is counted (`days_vacation` increments via the
W-5 leaves-table aggregation at line ~743) without a corresponding payment.

Reading the audit's H-06 headline in isolation ("sin remuneración
completa" / "no crea pago") would lead an implementer to conclude vacation
pay is entirely unimplemented and rewrite the money path from scratch —
that would be wasted/wrong-scoped work. The actual, narrower gap is: (a)
the rare overlay-less leave day produces no pay, and (b) — the part of H-06
that is unambiguously and fully true — there is **no seniority/anniversary
vacation-entitlement engine at all**. I grepped the entire backend for
`antiguedad|seniority|aniversario|anniversary|vacation_entitlement` and
found nothing: days-of-vacation-owed by tenure, annual increment, and a
formal vacation register (LOTTT Art. 190/192/203) simply do not exist as a
concept anywhere in the code. That part of H-06 is CONFIRMED at face value
and is a legal-completeness question (statutory entitlement schedule) that
I am not adjudicating — flagging it as outside my scope per your
instructions.

### If you were to act on this

Don't rewrite vacation pay computation — it exists and runs in the common
case. The real work is (1) closing the overlay-less-leave-day edge case
(e.g. by synthesizing the daily_records overlay row unconditionally at
leave-creation time rather than relying on recompute reaching it — the
comment at `reports/service.rs:613-618` already proposes exactly this), and
(2) building the seniority/anniversary entitlement + vacation register that
does not exist. Confirm the actual legal accrual rules with counsel before
implementing (2); I did not verify LOTTT Art. 190/192/203 requirements
myself.

---

## H-07 — Anulaciones aceptan estados inválidos y no recalculan coherentemente

**Verdict: CONFIRMED**

### Evidence

`backend/src/daily_records/handlers.rs` (`create_override`, current lines
69-201, was 103-151 at audit time — the hexagonal-adjacent file did not
move, only grew):

1. **Malformed timestamps silently become `None`** (lines 103-108,
   109-114):
   ```rust
   "override_entry_at" => {
       let val = field.text().await.unwrap_or_default();
       if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&val) {
           override_entry_at = Some(dt.timestamp());
       }
   }
   ```
   A garbage string for `override_entry_at` produces no error — it is
   indistinguishable from the field being omitted.

2. **No bounds on `override_work_minutes`** (line 115-118):
   ```rust
   "override_work_minutes" => {
       let val = field.text().await.unwrap_or_default();
       override_work_minutes = val.parse::<i64>().ok();
   }
   ```
   Any parseable `i64` is accepted — negative, zero, or absurdly large
   (e.g. 999999) values all pass straight into the audit-logged row and
   later into report money math as `effective_work_min`
   (`reports/service.rs:405`). I grepped
   `daily_records/handlers.rs`, `daily_records/service.rs`, and
   `reports/service.rs` for any range check on this field — there is none.
   Confirmed by the absence of any test in
   `backend/tests/daily_records_handlers_test.rs` asserting rejection of a
   negative or excessive value (the file's override-creation tests only
   cover auth, justification, evidence type/size, and entry/exit ordering).

3. **A no-op override is permitted**: `override_work_minutes`,
   `override_entry_at`, and `override_exit_at` are all `Option`, and there
   is no check requiring at least one to be `Some` before accepting the
   request — only `justification` and `evidence` are mandatory (lines
   159-175). An override that changes nothing but carries a justification +
   file is accepted and creates an audit-logged, revoking-of-the-prior-active-row
   event for zero actual effect.

4. **Order is validated only when both timestamps parse** (lines 180-187):
   ```rust
   if let (Some(entry), Some(exit)) = (override_entry_at, override_exit_at) {
       if exit <= entry { return Err(...) }
   }
   ```
   If one of the two is malformed (silently `None` per point 1) or omitted,
   this check never runs — an override can carry, e.g., a valid
   `override_exit_at` with no `override_entry_at`, with no coherence check
   against the record's actual entry/exit.

5. **Report money math uses `override_work_minutes` and ignores
   entry/exit, keeping the original overtime**: `reports/service.rs:405,
   486, 502-509` — `effective_work_min =
   override_work_min_opt.unwrap_or(work_minutes)` is used for ordinary pay,
   but `overtime_minutes` (line 400) is read straight from the
   engine-computed `daily_records` row, never recomputed from the override.
   The code **self-documents this as H-07** at
   `reports/service.rs:481-485`:
   ```rust
   // `effective_work_min` may come from an override while
   // `overtime_minutes` is the original engine value — hence the
   // `.max(0)`. The override not recomputing the overtime slice
   // is H-07, out of scope here; this comment is for the next
   // reader who wonders why the two can disagree.
   ```
   `override_entry_at`/`override_exit_at` are stored (and shown in the
   `OverrideResponse`/audit trail) but are **never read** anywhere else in
   the backend — I grepped `backend/src/**/*.rs` for both column names
   outside of `handlers.rs` (write) and found only the `SELECT` in
   `reports/service.rs:304` for `override_work_minutes`; the two timestamp
   columns appear nowhere else at all. They are pure audit-trail artifacts
   with zero computational effect.

Every element of the audit's H-07 description holds against current code,
line-for-line, with the added confirmation that the codebase itself now
carries a comment naming "H-07" as an open, acknowledged defect.

### If you were to act on this

The audit's recommendation (typed DTO with bounds, mandatory-motive
change-only overrides, full snapshot recompute under a versioned policy,
four-eyes for monetary impact) is directionally right. One trap for an
implementer: fixing "the override ignores entry/exit" by wiring
`override_entry_at`/`override_exit_at` into `effective_work_min` is not
enough by itself — `overtime_minutes`, `night_premium`, and
`rest_day_surcharge` are all computed independently from the *original*
`daily_records` row (`day_shift_type`, `is_rest_day_worked`, etc. in
`reports/service.rs:495-551`) and would still silently diverge from an
overridden work window unless the override triggers a real re-run of the
calc engine (`compute_daily_record`) against the overridden
entry/exit/minutes, not just a patched pay-formula input. The comment in
the code already flags this exact trap.
