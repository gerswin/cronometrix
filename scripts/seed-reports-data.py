#!/usr/bin/env python3
"""Seed daily_records covering every reportable scenario.

Idempotent — `ON CONFLICT(employee_id, anchor_date) DO UPDATE` so re-running
just refreshes the rows. Distribution is deterministic per (employee, day)
via md5 hash so re-runs produce the same dataset.

Scenarios covered (weekday / weekend × shift_type):

| Bucket           | Weight | Description                                    |
|------------------|--------|------------------------------------------------|
| on_time          | 30%    | 540 work, 0 late, 0 OT                         |
| late_within_tol  | 10%    | late=5 (within 10-min tolerance)               |
| late_moderate    | 8%     | late=25                                        |
| late_severe      | 4%     | late=90                                        |
| early_short      | 6%     | early=20                                       |
| early_severe     | 3%     | early=120                                      |
| ot_short         | 6%     | OT=30                                          |
| ot_moderate      | 6%     | OT=60                                          |
| ot_heavy         | 3%     | OT=180                                         |
| late_and_ot      | 3%     | late=20 + OT=60                                |
| late_and_early   | 2%     | late=30 + early=60                             |
| half_day         | 4%     | work=270 (4h30 — partial)                      |
| daily_cap_breach | 1%     | work=540 + OT=180 (LOTTT Art.178 cap >600)     |
| absent           | 8%     | work=0, no leave                               |
| absent_with_leave| 6%     | work=0 + leave row inserted                    |

Weekends (Sat/Sun):
| Bucket           | Weight | Description                                    |
|------------------|--------|------------------------------------------------|
| rest             | 90%    | no row                                         |
| rest_day_worked  | 10%    | 480 work, is_rest_day_worked=1, Sunday surcharge|

shift_type assignment (per-employee, deterministic):
- 75% day shift (entry 08:00)
- 15% night shift (entry 22:00, exit 06:00 next day)
- 10% mixed shift (rotates day↔night every 7 days)

Usage:
    python3 scripts/seed-reports-data.py                       # current month
    python3 scripts/seed-reports-data.py --month 2026-04
    python3 scripts/seed-reports-data.py --clear-month         # wipe before
    python3 scripts/seed-reports-data.py --month 2026-03 --clear-month
"""

import argparse
import datetime as dt
import hashlib
import os
import sqlite3
import sys
import uuid
from calendar import monthrange

CARACAS_TZ = dt.timezone(dt.timedelta(hours=-4))


def hash_bucket(*parts: str) -> int:
    return int(hashlib.md5("|".join(parts).encode()).hexdigest(), 16)


def assign_shift_type(emp_id: str) -> str:
    """Deterministic shift type per employee. 75% day, 15% night, 10% mixed."""
    b = hash_bucket(emp_id, "shift") % 100
    if b < 75:
        return "day"
    if b < 90:
        return "night"
    return "mixed"


def actual_shift_for_day(emp_shift: str, date_iso: str) -> str:
    """For mixed shift, rotate day↔night every 7 days. Otherwise return as-is."""
    if emp_shift != "mixed":
        return emp_shift
    week_idx = hash_bucket(date_iso, "week") % 2
    return "day" if week_idx == 0 else "night"


def workday_pattern(emp_id: str, date_iso: str) -> dict:
    """Pick a scenario from the weekday distribution. Deterministic per (emp, date)."""
    b = hash_bucket(emp_id, date_iso) % 100
    # Cumulative thresholds (must sum to 100):
    if b < 30:
        return dict(kind="on_time", work=540, late=0, ot=0, early=0)
    if b < 40:
        return dict(kind="late_within_tol", work=535, late=5, ot=0, early=0)
    if b < 48:
        return dict(kind="late_moderate", work=515, late=25, ot=0, early=0)
    if b < 52:
        return dict(kind="late_severe", work=450, late=90, ot=0, early=0)
    if b < 58:
        return dict(kind="early_short", work=520, late=0, ot=0, early=20)
    if b < 61:
        return dict(kind="early_severe", work=420, late=0, ot=0, early=120)
    if b < 67:
        return dict(kind="ot_short", work=540, late=0, ot=30, early=0)
    if b < 73:
        return dict(kind="ot_moderate", work=540, late=0, ot=60, early=0)
    if b < 76:
        return dict(kind="ot_heavy", work=540, late=0, ot=180, early=0)
    if b < 79:
        return dict(kind="late_and_ot", work=520, late=20, ot=60, early=0)
    if b < 81:
        return dict(kind="late_and_early", work=450, late=30, ot=0, early=60)
    if b < 85:
        return dict(kind="half_day", work=270, late=0, ot=0, early=270)
    if b < 86:
        # Daily cap breach — anomaly territory per LOTTT Art.178 (>600 total)
        return dict(kind="daily_cap_breach", work=540, late=0, ot=180, early=0)
    if b < 94:
        return dict(kind="absent", work=0, late=0, ot=0, early=0)
    return dict(kind="absent_with_leave", work=0, late=0, ot=0, early=0)


def weekend_pattern(emp_id: str, date_iso: str) -> dict | None:
    """10% of weekends are rest_day_worked. Returns None for normal rest day."""
    b = hash_bucket(emp_id, date_iso, "weekend") % 100
    if b < 90:
        return None
    return dict(kind="rest_day_worked", work=480, late=0, ot=0, early=0)


def compute_entry_exit(date: dt.date, shift_type: str, late: int, work: int, ot: int):
    """Return (entry_epoch, exit_epoch). Entry/exit times in Caracas local."""
    if work == 0:
        return None, None

    if shift_type == "day":
        # 08:00 + late, exit = entry + lunch(60) + work + ot
        entry_local = dt.datetime(
            date.year, date.month, date.day, 8, 0, tzinfo=CARACAS_TZ
        ) + dt.timedelta(minutes=late)
        exit_local = entry_local + dt.timedelta(minutes=60 + work + ot)
    elif shift_type == "night":
        # 22:00 + late, exit = entry + lunch(60) + work + ot (crosses midnight)
        entry_local = dt.datetime(
            date.year, date.month, date.day, 22, 0, tzinfo=CARACAS_TZ
        ) + dt.timedelta(minutes=late)
        exit_local = entry_local + dt.timedelta(minutes=60 + work + ot)
    else:
        # Should not reach here — mixed is resolved upstream into day/night
        entry_local = dt.datetime(
            date.year, date.month, date.day, 8, 0, tzinfo=CARACAS_TZ
        ) + dt.timedelta(minutes=late)
        exit_local = entry_local + dt.timedelta(minutes=60 + work + ot)

    return int(entry_local.timestamp()), int(exit_local.timestamp())


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--db", default="backend/cronometrix.db")
    p.add_argument(
        "--month",
        help="YYYY-MM (default: current month)",
        default=dt.date.today().strftime("%Y-%m"),
    )
    p.add_argument("--clear-month", action="store_true")
    p.add_argument(
        "--max-employees",
        type=int,
        default=None,
        help="Cap how many employees get records (helpful with 5k+ in db)",
    )
    args = p.parse_args()

    if not os.path.exists(args.db):
        print(f"ERROR: db not found: {args.db}", file=sys.stderr)
        return 1

    year, month = (int(x) for x in args.month.split("-"))
    days_in_month = monthrange(year, month)[1]

    con = sqlite3.connect(args.db)
    con.row_factory = sqlite3.Row
    cur = con.cursor()

    # Order by name so the cap is reproducible across runs
    cur.execute(
        "SELECT id, name, department_id FROM employees WHERE status = 'active' ORDER BY name"
    )
    employees = cur.fetchall()

    # Pick a real user id for leaves.created_by (NOT NULL FK to users.id)
    cur.execute("SELECT id FROM users ORDER BY id LIMIT 1")
    user_row = cur.fetchone()
    if user_row is None:
        print("ERROR: no users in db — needed for leaves.created_by FK", file=sys.stderr)
        return 1
    creator_id: str = user_row[0]
    if args.max_employees:
        employees = employees[: args.max_employees]
    if not employees:
        print("ERROR: no active employees in db", file=sys.stderr)
        return 1

    if args.clear_month:
        cur.execute(
            "DELETE FROM daily_records WHERE substr(anchor_date, 1, 7) = ?",
            (args.month,),
        )
        cleared_dr = cur.rowcount
        cur.execute(
            "DELETE FROM leaves WHERE substr(from_date, 1, 7) = ? AND id LIKE 'syn-leave-%'",
            (args.month,),
        )
        cleared_lv = cur.rowcount
        print(f"  cleared {cleared_dr} daily_records + {cleared_lv} synthetic leaves for {args.month}")

    now = int(dt.datetime.now().timestamp())
    inserted_dr = inserted_lv = 0
    counts: dict[str, int] = {}

    BATCH = 1000
    rows: list[tuple] = []
    leaves_rows: list[tuple] = []

    for day in range(1, days_in_month + 1):
        d = dt.date(year, month, day)
        date_iso = d.isoformat()
        is_weekend = d.weekday() >= 5

        for emp in employees:
            emp_shift = assign_shift_type(emp["id"])

            if is_weekend:
                pat = weekend_pattern(emp["id"], date_iso)
                if pat is None:
                    counts["rest"] = counts.get("rest", 0) + 1
                    continue
                shift_type = "day"  # rest-day-worked uses day-shift hours by default
                kind = pat["kind"]
            else:
                pat = workday_pattern(emp["id"], date_iso)
                shift_type = actual_shift_for_day(emp_shift, date_iso)
                kind = pat["kind"]

            counts[kind] = counts.get(kind, 0) + 1

            entry_epoch, exit_epoch = compute_entry_exit(
                d, shift_type, pat["late"], pat["work"], pat["ot"]
            )

            leave_id = None
            if kind == "absent_with_leave":
                leave_id = f"syn-leave-{emp['id']}-{date_iso}"
                # Pick one of the 4 leave types in rotation by hash
                ltype_b = hash_bucket(emp["id"], date_iso, "ltype") % 4
                ltype = ["medical", "vacation", "unpaid", "manual"][ltype_b]
                leaves_rows.append((
                    leave_id,
                    emp["id"],
                    date_iso,
                    date_iso,
                    ltype,
                    "Justificación generada por seed",
                    None,  # evidence_path
                    creator_id,
                    "active",
                    None,  # deleted_at
                    1,
                    now,
                    now,
                ))

            rest_worked = 1 if kind == "rest_day_worked" else 0

            rows.append((
                str(uuid.uuid4()),
                emp["id"],
                emp["department_id"],
                date_iso,
                shift_type,
                pat["work"],
                pat["ot"],
                pat["late"],
                pat["early"],
                rest_worked,
                entry_epoch,
                exit_epoch,
                leave_id,
                now,
                now,
                now,
            ))

            if len(rows) >= BATCH:
                cur.executemany(
                    """
                    INSERT INTO daily_records (
                        id, employee_id, department_id, anchor_date, shift_type,
                        work_minutes, overtime_minutes, late_minutes,
                        early_departure_minutes, is_rest_day_worked,
                        entry_at, exit_at, leave_id, computed_at, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(employee_id, anchor_date) DO UPDATE SET
                        department_id = excluded.department_id,
                        shift_type = excluded.shift_type,
                        work_minutes = excluded.work_minutes,
                        overtime_minutes = excluded.overtime_minutes,
                        late_minutes = excluded.late_minutes,
                        early_departure_minutes = excluded.early_departure_minutes,
                        is_rest_day_worked = excluded.is_rest_day_worked,
                        entry_at = excluded.entry_at,
                        exit_at = excluded.exit_at,
                        leave_id = excluded.leave_id,
                        computed_at = excluded.computed_at,
                        updated_at = excluded.updated_at
                    """,
                    rows,
                )
                inserted_dr += cur.rowcount
                rows.clear()

    # Flush remaining
    if rows:
        cur.executemany(
            """
            INSERT INTO daily_records (
                id, employee_id, department_id, anchor_date, shift_type,
                work_minutes, overtime_minutes, late_minutes,
                early_departure_minutes, is_rest_day_worked,
                entry_at, exit_at, leave_id, computed_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(employee_id, anchor_date) DO UPDATE SET
                department_id = excluded.department_id,
                shift_type = excluded.shift_type,
                work_minutes = excluded.work_minutes,
                overtime_minutes = excluded.overtime_minutes,
                late_minutes = excluded.late_minutes,
                early_departure_minutes = excluded.early_departure_minutes,
                is_rest_day_worked = excluded.is_rest_day_worked,
                entry_at = excluded.entry_at,
                exit_at = excluded.exit_at,
                leave_id = excluded.leave_id,
                computed_at = excluded.computed_at,
                updated_at = excluded.updated_at
            """,
            rows,
        )
        inserted_dr += cur.rowcount

    if leaves_rows:
        # Insert leaves with INSERT OR IGNORE (idempotent on rerun)
        cur.executemany(
            """
            INSERT OR IGNORE INTO leaves (
                id, employee_id, from_date, to_date,
                leave_type, justification, evidence_path,
                created_by, status, deleted_at,
                version, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            leaves_rows,
        )
        inserted_lv = cur.rowcount

    con.commit()
    con.close()

    print(f"\nSeed complete for {args.month}")
    print(f"  daily_records inserted/updated : {inserted_dr:,}")
    print(f"  synthetic leaves inserted      : {inserted_lv:,}")
    print(f"  employees                      : {len(employees):,}")
    print(f"\n  Scenario distribution:")
    for k in sorted(counts, key=lambda k: -counts[k]):
        pct = counts[k] / sum(counts.values()) * 100
        print(f"    {k:<22} {counts[k]:>7,}  ({pct:5.2f}%)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
