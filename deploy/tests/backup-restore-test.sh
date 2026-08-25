#!/usr/bin/env bash
# Bloque 3 (H-14): docker-free contract test for the evidence backup/restore.
#
# "A backup that has never been restored is an assumption." This proves the
# round-trip on a real filesystem: back up DB + evidence, destroy the live data,
# restore, and assert the images come back — plus that the manifest detects a
# DB/files mismatch.
set -Eeuo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy/lib/evidence-backup.sh
source "${HERE}/../lib/evidence-backup.sh"

fail() { echo "FAIL: $*" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

DATA_DIR="${WORK}/data"
BACKUP_DIR="${WORK}/backup"
mkdir -p "${DATA_DIR}" "${BACKUP_DIR}"

# ---- seed a live install: a DB and evidence under each evidence dir ----------
printf 'SQLite format 3\0live-db' > "${DATA_DIR}/cronometrix.db"
mkdir -p "${DATA_DIR}/enrollments/emp-1" "${DATA_DIR}/events/emp-1" \
         "${DATA_DIR}/leaves" "${DATA_DIR}/overrides"
printf 'face' > "${DATA_DIR}/enrollments/emp-1/face.jpg"
printf 'punch' > "${DATA_DIR}/events/emp-1/punch.jpg"
printf '%%PDF-1.4' > "${DATA_DIR}/leaves/leave.pdf"
printf 'override' > "${DATA_DIR}/overrides/o.json"

# ---- backup: DB first (as install.sh does), then the evidence + manifest -----
cp "${DATA_DIR}/cronometrix.db" "${BACKUP_DIR}/cronometrix.db"
backup_evidence_dirs "${DATA_DIR}" "${BACKUP_DIR}"
write_backup_manifest "${BACKUP_DIR}"

[[ -f "${BACKUP_DIR}/enrollments/emp-1/face.jpg" ]] || fail "enrollment evidence was not backed up"
[[ -f "${BACKUP_DIR}/events/emp-1/punch.jpg" ]]     || fail "event evidence was not backed up"
[[ -f "${BACKUP_DIR}/leaves/leave.pdf" ]]           || fail "leave evidence was not backed up"
[[ -f "${BACKUP_DIR}/overrides/o.json" ]]           || fail "override evidence was not backed up"
[[ -f "${BACKUP_DIR}/backup-manifest.txt" ]]        || fail "manifest was not written"

verify_backup_manifest "${BACKUP_DIR}" || fail "a fresh backup must verify clean"

# ---- disaster: lose ALL live data (DB and every image) -----------------------
rm -rf "${DATA_DIR}"
mkdir -p "${DATA_DIR}"
cp "${BACKUP_DIR}/cronometrix.db" "${DATA_DIR}/cronometrix.db"   # DB-only restore (the old behaviour)

# Before Task 3 this is where recovery stopped — DB back, images gone:
[[ ! -e "${DATA_DIR}/events/emp-1/punch.jpg" ]] || fail "precondition: images should be gone before evidence restore"

# ---- restore the evidence directories ----------------------------------------
restore_evidence_dirs "${BACKUP_DIR}" "${DATA_DIR}"

[[ "$(cat "${DATA_DIR}/enrollments/emp-1/face.jpg")" == "face" ]]  || fail "enrollment image did not come back intact"
[[ "$(cat "${DATA_DIR}/events/emp-1/punch.jpg")" == "punch" ]]     || fail "event image did not come back intact"
[[ "$(cat "${DATA_DIR}/leaves/leave.pdf")" == "%PDF-1.4" ]]        || fail "leave evidence did not come back intact"
[[ "$(cat "${DATA_DIR}/overrides/o.json")" == "override" ]]        || fail "override did not come back intact"

# ---- the manifest must catch a DB/files mismatch -----------------------------
printf 'tampered' > "${BACKUP_DIR}/cronometrix.db"
if verify_backup_manifest "${BACKUP_DIR}" 2>/dev/null; then
    fail "manifest verification should reject a backup whose DB no longer matches"
fi

echo "PASS: evidence backup/restore round-trip (H-14)"
