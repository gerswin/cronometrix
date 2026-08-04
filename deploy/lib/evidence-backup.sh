#!/usr/bin/env bash
# Bloque 3 (H-14): back up and restore the attendance-evidence directories
# alongside the SQLite database, with a manifest that lets a restore detect a
# DB/files mismatch.
#
# Why this exists: before this, a backup covered only cronometrix.db, so a
# restore recovered the database and lost every face/event/leave image — the DB
# rows then point at files that no longer exist. That is not degradation, it is
# loss of proof-of-work (H-09).
#
# Sourced by deploy/install.sh (the upgrade backup/rollback path) and by
# deploy/tests/backup-restore-test.sh (a docker-free round-trip contract test).
# It defines functions only — sourcing it runs nothing.

# The evidence directories, named relative to DATA_DIR. These mirror the roots
# in backend/src/state/paths.rs (enrollments / events / leaves / overrides).
EVIDENCE_DIRS=(enrollments events leaves overrides)

# _sha256 FILE — print the file's SHA-256, or a sentinel if no tool is present.
_sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        echo "no-sha256-tool"
    fi
}

# backup_evidence_dirs DATA_DIR BACKUP_DIR
# Copy each existing evidence directory into BACKUP_DIR/<name>.
#
# ORDER CONTRACT: the caller MUST snapshot the DB into BACKUP_DIR FIRST, then
# call this. DB-first makes a concurrent evidence *creation* at worst a harmless
# orphan file on restore (a file with no DB row), never the dangerous case (a DB
# row with no file). For a fully consistent live backup the deletion workers
# (retention sweep, termination purge) should additionally be quiesced for the
# backup window; the manifest below is the detect-and-reject backstop.
backup_evidence_dirs() {
    local data_dir="$1" backup_dir="$2" d src
    for d in "${EVIDENCE_DIRS[@]}"; do
        src="${data_dir}/${d}"
        if [[ -d "$src" ]]; then
            cp -a "$src" "${backup_dir}/${d}"
        fi
    done
}

# restore_evidence_dirs BACKUP_DIR DATA_DIR
# Restore each backed-up evidence directory, replacing the live one via a
# temp-then-mv swap so a crash mid-restore never leaves a half-copied directory
# in place.
restore_evidence_dirs() {
    local backup_dir="$1" data_dir="$2" d bak dst tmp
    for d in "${EVIDENCE_DIRS[@]}"; do
        bak="${backup_dir}/${d}"
        [[ -d "$bak" ]] || continue
        dst="${data_dir}/${d}"
        tmp="${dst}.restore-tmp.$$"
        rm -rf "$tmp"
        cp -a "$bak" "$tmp"
        rm -rf "$dst"
        mv "$tmp" "$dst"
    done
}

# write_backup_manifest BACKUP_DIR
# Record a UTC stamp, the backed-up DB's SHA-256 and size, and the evidence dirs
# present, so a restore can prove the DB and the files came from one backup.
write_backup_manifest() {
    local backup_dir="$1" d
    local manifest="${backup_dir}/backup-manifest.txt"
    local db="${backup_dir}/cronometrix.db"
    {
        printf 'schema=1\n'
        printf 'created_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        if [[ -f "$db" ]]; then
            printf 'db_sha256=%s\n' "$(_sha256 "$db")"
            printf 'db_bytes=%s\n' "$(wc -c < "$db" | tr -d ' ')"
        else
            printf 'db_sha256=absent\n'
            printf 'db_bytes=0\n'
        fi
        for d in "${EVIDENCE_DIRS[@]}"; do
            if [[ -d "${backup_dir}/${d}" ]]; then
                printf 'evidence_dir=%s\n' "$d"
            fi
        done
    } > "$manifest"
}

# verify_backup_manifest BACKUP_DIR
# Return non-zero if the manifest is missing or its recorded DB SHA-256 no longer
# matches the DB present in the backup — i.e. the backup is internally
# inconsistent and must not be trusted for a restore. A base restored without its
# files looks like it works, which is worse than a restore that refuses.
verify_backup_manifest() {
    local backup_dir="$1"
    local manifest="${backup_dir}/backup-manifest.txt"
    local db="${backup_dir}/cronometrix.db"
    local recorded actual
    if [[ ! -f "$manifest" ]]; then
        echo "backup manifest missing at ${manifest}" >&2
        return 1
    fi
    recorded="$(awk -F= '/^db_sha256=/{print $2}' "$manifest")"
    if [[ "$recorded" == "absent" ]]; then
        if [[ -f "$db" ]]; then
            echo "manifest records no DB but a DB is present in the backup" >&2
            return 1
        fi
        return 0
    fi
    if [[ ! -f "$db" ]]; then
        echo "manifest records a DB but none is present in the backup" >&2
        return 1
    fi
    actual="$(_sha256 "$db")"
    if [[ "$recorded" != "$actual" ]]; then
        echo "backup DB sha256 mismatch: manifest=${recorded} actual=${actual}" >&2
        return 1
    fi
    return 0
}
