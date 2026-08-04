//! Bloque 3 (H-10): configurable retention sweep over the attendance-evidence
//! directories.
//!
//! MECHANISM, NOT POLICY. The concrete retention periods depend on an unanswered
//! labour consultation, so they live in the `retention_policy` table rather than
//! being wired here, and the safe default (NULL = keep forever) means this sweep
//! is INERT until an operator sets a period. A default that deletes would turn an
//! unattended deployment into loss of proof-of-work (H-09); a default that keeps
//! only costs disk.
//!
//! The sweep is deliberately conservative:
//!   * Only files whose modified-time exceeds the configured period are deleted.
//!   * Only regular `.jpg` evidence files are eligible — deletion goes through
//!     the `storage` module's path/ownership/symlink validations, which are
//!     JPEG-scoped. Any other file (e.g. a leave PDF, raw XML) is kept, never
//!     raw-unlinked.
//!   * Symlinks and special files are skipped, never followed.
//!
//! Without period closure (H-09, outside this block) there is no reliable
//! "is this period still open?" signal, so the sweep errs toward keeping.

use std::path::Path;
use std::time::{Duration, SystemTime};

use tokio_util::sync::CancellationToken;

use crate::state::AppState;
use crate::storage::atomic_file::{inspect_owned_file, remove_owned_file};

/// Daily cadence. The sweep is idempotent, so a missed tick is harmless.
pub const RETENTION_CADENCE: Duration = Duration::from_secs(24 * 60 * 60);

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RetentionStats {
    pub deleted: usize,
    pub kept: usize,
    pub skipped_other: usize,
    pub errors: usize,
}

struct RetentionPolicy {
    events_retention_days: Option<i64>,
    leaves_retention_days: Option<i64>,
}

/// Long-lived worker: sweep once per cadence until shutdown. Errors are logged,
/// never fatal — a failed sweep must not take the process down.
pub async fn run(state: AppState, shutdown: CancellationToken) {
    run_with_cadence(state, shutdown, RETENTION_CADENCE).await
}

/// `run` with an injectable cadence so tests can drive a real (short) interval
/// deterministically instead of racing a paused clock against real filesystem
/// and DB I/O.
pub async fn run_with_cadence(state: AppState, shutdown: CancellationToken, cadence: Duration) {
    tracing::info!("retention sweep worker started");
    let mut interval = tokio::time::interval(cadence);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => {
                tracing::info!("retention sweep worker shutting down");
                return;
            }
            _ = interval.tick() => {
                match sweep_once(&state, SystemTime::now()).await {
                    Ok(stats) if stats.deleted > 0 || stats.errors > 0 => {
                        tracing::info!(
                            deleted = stats.deleted, kept = stats.kept,
                            skipped_other = stats.skipped_other, errors = stats.errors,
                            "retention sweep complete"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => tracing::error!(err = %e, "retention sweep failed"),
                }
            }
        }
    }
}

/// One sweep. `now` is injected so tests can control file age deterministically
/// without touching the wall clock.
pub async fn sweep_once(state: &AppState, now: SystemTime) -> anyhow::Result<RetentionStats> {
    let policy = load_policy(state).await?;
    let mut stats = RetentionStats::default();

    if let Some(days) = policy.events_retention_days {
        sweep_dir(state, &state.paths.events_root, "events", days, now, &mut stats).await?;
    }
    if let Some(days) = policy.leaves_retention_days {
        sweep_dir(state, &state.paths.leaves_root, "leaves", days, now, &mut stats).await?;
    }
    Ok(stats)
}

async fn load_policy(state: &AppState) -> anyhow::Result<RetentionPolicy> {
    let conn = state.db.connect()?;
    let mut rows = conn
        .query(
            "SELECT events_retention_days, leaves_retention_days FROM retention_policy WHERE id = 1",
            (),
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(RetentionPolicy {
            events_retention_days: row.get::<Option<i64>>(0)?,
            leaves_retention_days: row.get::<Option<i64>>(1)?,
        }),
        // No row → treat as the safe default: keep everything.
        None => Ok(RetentionPolicy {
            events_retention_days: None,
            leaves_retention_days: None,
        }),
    }
}

async fn sweep_dir(
    state: &AppState,
    root: &Path,
    class: &str,
    retention_days: i64,
    now: SystemTime,
    stats: &mut RetentionStats,
) -> anyhow::Result<()> {
    // A non-positive period is treated as "keep forever" — never interpret a
    // misconfiguration as "delete everything".
    if retention_days <= 0 {
        return Ok(());
    }
    let window = Duration::from_secs((retention_days as u64).saturating_mul(SECONDS_PER_DAY));
    let cutoff = match now.checked_sub(window) {
        Some(c) => c,
        None => return Ok(()), // window reaches before the epoch → nothing qualifies
    };

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut read_dir = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(_) => continue, // missing/unreadable dir → nothing to sweep here
        };
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let file_type = match entry.file_type().await {
                Ok(t) => t,
                Err(_) => {
                    stats.errors += 1;
                    continue;
                }
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                // symlink or special file — never follow, never delete.
                stats.skipped_other += 1;
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
                // Non-JPEG evidence has no validated deletion path yet — keep it
                // rather than raw-unlink.
                stats.skipped_other += 1;
                continue;
            }
            let Some(parent) = path.parent() else {
                stats.skipped_other += 1;
                continue;
            };
            match inspect_owned_file(parent, &path) {
                Ok(insp) => {
                    if insp.modified() <= cutoff {
                        match remove_owned_file(parent, &path, insp.identity()) {
                            Ok(()) => {
                                stats.deleted += 1;
                                audit_deletion(state, class, &path, retention_days).await;
                            }
                            Err(e) => {
                                stats.errors += 1;
                                tracing::warn!(class, path = %path.display(), err = %e, "retention: delete failed");
                            }
                        }
                    } else {
                        stats.kept += 1;
                    }
                }
                Err(e) => {
                    stats.errors += 1;
                    tracing::warn!(class, path = %path.display(), err = %e, "retention: inspect failed");
                }
            }
        }
    }
    Ok(())
}

async fn audit_deletion(state: &AppState, class: &str, path: &Path, retention_days: i64) {
    let audit_id = uuid::Uuid::new_v4().to_string();
    let old_data = serde_json::json!({
        "reason": "retention period exceeded (H-10)",
        "class": class,
        "retention_days": retention_days,
        "path": path.display().to_string(),
    })
    .to_string();
    if let Err(e) = state
        .db_write
        .statement(
            "retention.delete-audit",
            "INSERT INTO audit_log \
             (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at) \
             VALUES (?1, ?2, ?3, 'DELETE', ?4, NULL, NULL, unixepoch())",
            vec![
                libsql::Value::Text(audit_id),
                libsql::Value::Text(class.to_string()),
                libsql::Value::Text(path.display().to_string()),
                libsql::Value::Text(old_data),
            ],
        )
        .await
    {
        tracing::error!(class, path = %path.display(), err = %e, "retention: failed to audit deletion");
    }
}
