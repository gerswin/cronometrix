#!/usr/bin/env python3
"""Seed N synthetic employees, evenly distributed across active departments.

Usage:
    python3 scripts/seed-employees.py                      # default 5000
    python3 scripts/seed-employees.py --count 10000
    python3 scripts/seed-employees.py --count 5000 --clear-synthetic
        # wipe employees with id starting with 'syn-' before re-seeding

Idempotent re-runs are achieved via deterministic ids `syn-<NNNNNN>` so a re-run
does nothing new (employee_code is also derived deterministically). Use
`--clear-synthetic` to wipe and re-create with a different distribution.
"""

import argparse
import datetime as dt
import os
import random
import sqlite3
import sys
import uuid

FIRST_NAMES = [
    "José", "María", "Luis", "Carmen", "Juan", "Ana", "Carlos", "Rosa", "Pedro",
    "Lucía", "Miguel", "Isabel", "Jorge", "Patricia", "Manuel", "Sofía", "Ricardo",
    "Elena", "Francisco", "Andrea", "Diego", "Gabriela", "Antonio", "Valeria",
    "Rafael", "Daniela", "Eduardo", "Alejandra", "Fernando", "Mariana", "Roberto",
    "Adriana", "Andrés", "Beatriz", "Hugo", "Verónica", "Iván", "Cristina",
    "Sebastián", "Camila", "Esteban", "Natalia", "Tomás", "Paula", "Nicolás",
    "Lorena", "Javier", "Yolanda", "Raúl", "Silvia",
]

LAST_NAMES = [
    "González", "Rodríguez", "Martínez", "García", "Hernández", "López", "Pérez",
    "Sánchez", "Ramírez", "Torres", "Flores", "Rivera", "Gómez", "Díaz", "Reyes",
    "Morales", "Cruz", "Ortiz", "Gutiérrez", "Chávez", "Ramos", "Ruiz", "Vargas",
    "Castillo", "Jiménez", "Mendoza", "Romero", "Suárez", "Álvarez", "Aguilar",
    "Mendez", "Herrera", "Castro", "Vega", "Rojas", "Medina", "Cordero", "Bravo",
    "Salazar", "Arias", "Carrillo", "Núñez", "Acosta", "Peña", "Cabrera",
    "Domínguez", "Soto", "Espinoza", "Silva", "Padilla",
]

POSITIONS = [
    "", "", "",  # weight blanks higher; position is optional in seeds
    "Operario", "Supervisor", "Analista", "Asistente", "Técnico",
    "Coordinador", "Especialista", "Gerente Junior",
]


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--db", default="backend/cronometrix.db")
    p.add_argument("--count", type=int, default=5000)
    p.add_argument("--clear-synthetic", action="store_true",
                   help="DELETE synthetic (id LIKE syn-...) employees before seeding")
    p.add_argument("--seed", type=int, default=42, help="random seed for reproducibility")
    args = p.parse_args()

    if not os.path.exists(args.db):
        print(f"ERROR: db not found: {args.db}", file=sys.stderr)
        return 1

    random.seed(args.seed)

    con = sqlite3.connect(args.db)
    con.row_factory = sqlite3.Row
    cur = con.cursor()

    cur.execute(
        "SELECT id, name, base_salary_cents FROM departments WHERE status = 'active' OR status IS NULL"
    )
    depts = cur.fetchall()
    if not depts:
        print("ERROR: no active departments — create some first", file=sys.stderr)
        return 1

    if args.clear_synthetic:
        cur.execute("DELETE FROM employees WHERE id LIKE 'syn-%'")
        print(f"  cleared {cur.rowcount} synthetic employees")

    # Get next employee_code suffix to avoid UNIQUE collision with EMP001..EMP006
    # already in seed_e2e. Synthetic codes use a SYN prefix so no overlap.
    now = int(dt.datetime.now().timestamp())
    # Hire dates spread across the last 5 years
    five_years_ago = now - 5 * 365 * 86400

    inserted = skipped = 0
    BATCH = 500
    rows: list[tuple] = []

    for i in range(1, args.count + 1):
        emp_id = f"syn-{i:06d}"
        emp_code = f"SYN{i:06d}"

        # Skip if already exists (idempotent — fast path before INSERT OR IGNORE)
        # Bulk INSERT OR IGNORE handles duplicates implicitly anyway.
        first = random.choice(FIRST_NAMES)
        last = random.choice(LAST_NAMES)
        last2 = random.choice(LAST_NAMES)
        name = f"{first} {last} {last2}"

        dept = random.choice(depts)
        position = random.choice(POSITIONS)

        # ~85% have a hire_date, the rest are null (matches realistic data: not all
        # hire dates are recorded)
        hire_date: int | None = None
        if random.random() < 0.85:
            hire_date = random.randint(five_years_ago, now - 30 * 86400)

        # Per-employee salary: jitter dept base ±15% so payroll spreads realistic.
        # Falls back to ~100M cents ($1M VES) if department has no base.
        dept_base = dept["base_salary_cents"] or 100_000_000
        salary = int(dept_base * random.uniform(0.85, 1.15))

        rows.append((
            emp_id, emp_code, name, dept["id"], "active", None,
            1, position, hire_date, now, now, salary,
        ))

        if len(rows) >= BATCH:
            cur.executemany(
                """
                INSERT OR IGNORE INTO employees (
                    id, employee_code, name, department_id, status, deleted_at,
                    version, position, hire_date, created_at, updated_at, base_salary_cents
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                rows,
            )
            inserted += cur.rowcount
            skipped += BATCH - cur.rowcount
            rows.clear()
            if i % 1000 == 0:
                print(f"  ...{i:,} of {args.count:,}")

    if rows:
        cur.executemany(
            """
            INSERT OR IGNORE INTO employees (
                id, employee_code, name, department_id, status, deleted_at,
                version, position, hire_date, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            rows,
        )
        inserted += cur.rowcount
        skipped += len(rows) - cur.rowcount

    con.commit()

    cur.execute("SELECT COUNT(*) FROM employees WHERE id LIKE 'syn-%'")
    total_syn = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM employees")
    total_all = cur.fetchone()[0]

    con.close()

    print(f"\nSeed complete")
    print(f"  inserted now    : {inserted:,}")
    print(f"  skipped (dupes) : {skipped:,}")
    print(f"  total synthetic : {total_syn:,}")
    print(f"  total employees : {total_all:,}")
    print(f"  departments     : {len(depts)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
