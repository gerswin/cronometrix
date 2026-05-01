#!/usr/bin/env python3
"""Create a Cronometrix user directly in SQLite.

Usage:
  python3 scripts/create-user.py <username> <full_name> <role> <password>

Roles: admin | supervisor | viewer

Requires: pip install argon2-cffi

Note: stop the API container before running to avoid WAL contention:
  docker compose -f deploy/docker-compose.local.yml stop api
  python3 scripts/create-user.py jdoe "John Doe" supervisor "Secret123."
  docker compose -f deploy/docker-compose.local.yml start api
"""
import sys
import sqlite3
import time
import uuid
from argon2 import PasswordHasher

DB_PATH = "deploy/data/cronometrix.db"
VALID_ROLES = {"admin", "supervisor", "viewer"}

def main():
    if len(sys.argv) != 5:
        print(__doc__)
        sys.exit(1)
    username, full_name, role, password = sys.argv[1:]
    if role not in VALID_ROLES:
        print(f"ERROR: role must be one of {VALID_ROLES}")
        sys.exit(2)
    if len(password) < 8:
        print("ERROR: password must be at least 8 chars")
        sys.exit(2)

    ph = PasswordHasher()  # argon2id default; PHC string compatible with RustCrypto
    pwd_hash = ph.hash(password)
    user_id = str(uuid.uuid4())
    now = int(time.time())

    conn = sqlite3.connect(DB_PATH)
    try:
        conn.execute(
            "INSERT INTO users "
            "(id, username, full_name, password_hash, role, status, version, created_at, updated_at) "
            "VALUES (?, ?, ?, ?, ?, 'active', 1, ?, ?)",
            (user_id, username, full_name, pwd_hash, role, now, now),
        )
        conn.commit()
        print(f"OK created user {username} ({role}) id={user_id}")
    except sqlite3.IntegrityError as e:
        print(f"ERROR: {e} (username already exists?)")
        sys.exit(3)
    finally:
        conn.close()

if __name__ == "__main__":
    main()
