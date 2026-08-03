//! Drains `device_push_inbox`, turning stored pushes into attendance events
//! off the HTTP response path (C-10, part 2).
//!
//! `devices/push.rs::receive_push` answers 200 the instant the raw body is
//! durable — it no longer parses or ingests anything itself. This worker is
//! what actually turns a stored row into an attendance event, on its own
//! cadence, with its own retry budget. See `backend/src/workers/capture_cleanup.rs`
//! for the cadence/shutdown-signal shape this file imitates.
//!
//! **Transient vs permanent is the whole design here.** A body that will
//! never parse must not be retried forever — that would loop indefinitely
//! and clog the queue behind it (FIFO by `received_at`). A database that is
//! momentarily busy must not be given up on after one try — that loses the
//! marking exactly as C-10 originally did, just one layer down. When a
//! failure cannot be classified with confidence, it is treated as transient:
//! erring toward retry costs work, erring toward discard costs a marking.

use std::time::Duration;

use bytes::Bytes;
use libsql::params;
use tokio_util::sync::CancellationToken;

use crate::devices::push::split_payload;
use crate::isapi::ingest::{ingest_alert, IngestOutcome};
use crate::state::AppState;

/// How often the worker looks for pending rows.
pub const DRAIN_CADENCE: Duration = Duration::from_secs(3);

/// Rows selected per pass. Bounded so one pass cannot starve the write queue
/// or hold the read connection for an unbounded scan under a large backlog.
pub const DRAIN_BATCH_SIZE: i64 = 50;

/// Ceiling on retries for a transient failure. Past this, a row that keeps
/// hitting a busy database is dead-lettered too: an unbounded retry queue
/// behind a persistently overloaded writer would grow forever, which is its
/// own kind of silent failure — nothing observable, nothing draining, and
/// every push behind it starved by FIFO order.
pub const MAX_ATTEMPTS: i64 = 10;

/// Longest `last_error` text stored. The underlying column is unbounded, but
/// an `anyhow` chain from a deeply-wrapped error can run long; this keeps the
/// dead-letter view readable rather than silently defensive against nothing.
const MAX_ERROR_LEN: usize = 2000;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DrainStats {
    /// Pending rows looked at this pass.
    pub selected: usize,
    /// Rows that ended in `done`.
    pub done: usize,
    /// Rows that stayed `pending` with `attempts` incremented.
    pub retried: usize,
    /// Rows that ended in `failed` (dead-lettered), whether because the
    /// failure was permanent or because `attempts` reached `MAX_ATTEMPTS`.
    pub failed: usize,
}

struct PendingRow {
    id: String,
    device_id: String,
    content_type: String,
    body: Vec<u8>,
    attempts: i64,
    /// The owning device's `direction` column, used as `ingest_alert`'s
    /// `direction_default`. `None` when the device row is gone — a permanent
    /// failure, since nothing will bring it back.
    direction_default: Option<String>,
}

/// How processing one row came out, before it is translated into a row
/// mutation. Kept separate from the mutation so the classification itself —
/// the part this task is actually about — is a small, direct match with no
/// database plumbing in the way.
enum RowOutcome {
    Done,
    Permanent(String),
    Transient(String),
}

/// One drain pass: select up to `DRAIN_BATCH_SIZE` pending rows oldest-first
/// and resolve each to `done`, `pending` (retried), or `failed`.
pub async fn drain_once(state: &AppState) -> anyhow::Result<DrainStats> {
    let mut stats = DrainStats::default();
    let rows = select_pending(state, DRAIN_BATCH_SIZE).await?;

    for row in rows {
        stats.selected += 1;
        let outcome = process_row(state, &row).await;
        resolve(state, &row, outcome, &mut stats).await;
    }

    Ok(stats)
}

/// Parse and ingest every part of one stored body, and classify the result.
///
/// Calls `split_payload` (owned by `devices::push`) and `ingest_alert` (owned
/// by `isapi::ingest`) exactly as the handler used to — this function does
/// not parse the body itself and does not construct a `RawMarking`. That
/// stays inside `isapi/ingest.rs`, on the other side of the hexagonal
/// boundary.
async fn process_row(state: &AppState, row: &PendingRow) -> RowOutcome {
    let Some(direction_default) = row.direction_default.as_deref() else {
        // The device row is gone (deleted after the push landed in the
        // inbox). No default direction exists to attribute the event to, and
        // no retry brings the device row back — permanent.
        return RowOutcome::Permanent(format!(
            "device {} no longer exists; cannot resolve a direction default",
            row.device_id
        ));
    };

    let body = Bytes::from(row.body.clone());
    let parts = split_payload(&row.content_type, &body);

    // A row can carry more than one part (alert + JPEG pairs). Transient
    // takes priority over permanent: a genuinely unparseable part parses no
    // differently on a retry, so once a transient condition clears, a later
    // pass converges on the correct permanent verdict anyway. Retrying costs
    // time; discarding early costs the marking.
    let mut transient: Option<String> = None;
    let mut permanent: Option<String> = None;

    for (part_content_type, alert_bytes, jpeg) in parts {
        let raw = String::from_utf8_lossy(&alert_bytes).to_string();
        match ingest_alert(
            state,
            &row.device_id,
            direction_default,
            &alert_bytes,
            &part_content_type,
            raw,
            jpeg,
        )
        .await
        {
            // Persisted, Deduplicated, or a legitimate Skipped (heartbeat,
            // door/tamper, no identity) are all successful outcomes: the part
            // was interpreted correctly, whether or not it produced a row.
            Ok(IngestOutcome::Unparseable) => {
                permanent.get_or_insert_with(|| {
                    "part could not be parsed as a Hikvision alert".to_string()
                });
            }
            Ok(_) => {}
            Err(error) => {
                // Every `Err` path reachable from `ingest_alert` today
                // originates in the database write queue (busy, closed, or a
                // read ahead of it) — there is no business-rejection `Err`
                // path in this codebase yet. Since the error has already been
                // stringified by the time it reaches here, it cannot be
                // reliably classified further than "not a parse failure".
                // Per the brief: an unclassifiable failure is transient.
                transient.get_or_insert_with(|| error.to_string());
            }
        }
    }

    if let Some(reason) = transient {
        RowOutcome::Transient(reason)
    } else if let Some(reason) = permanent {
        RowOutcome::Permanent(reason)
    } else {
        RowOutcome::Done
    }
}

/// Translate a classification into the row's next state and persist it.
///
/// A write failure here (e.g. the queue is busy right when we try to record
/// the outcome) is logged and the row is left exactly as `select_pending`
/// found it — still `pending` at its prior `attempts` count. The next drain
/// pass re-selects it and tries again; re-running `process_row` is safe
/// because `ingest_alert` is idempotent (attendance_events dedups on
/// `bucket_30s`), so a row is never double-counted for having been retried.
async fn resolve(state: &AppState, row: &PendingRow, outcome: RowOutcome, stats: &mut DrainStats) {
    let result = match outcome {
        RowOutcome::Done => {
            stats.done += 1;
            mark_done(state, &row.id).await
        }
        RowOutcome::Permanent(reason) => {
            stats.failed += 1;
            mark_failed(state, &row.id, row.attempts, &reason).await
        }
        RowOutcome::Transient(reason) => {
            let attempts = row.attempts + 1;
            if attempts >= MAX_ATTEMPTS {
                stats.failed += 1;
                mark_failed(
                    state,
                    &row.id,
                    attempts,
                    &format!("giving up after {attempts} attempts: {reason}"),
                )
                .await
            } else {
                stats.retried += 1;
                mark_retry(state, &row.id, attempts, &reason).await
            }
        }
    };

    if let Err(error) = result {
        tracing::error!(
            inbox_id = %row.id,
            device_id = %row.device_id,
            err = %error,
            "push inbox status update failed; row left pending for the next pass"
        );
    }
}

async fn select_pending(state: &AppState, limit: i64) -> anyhow::Result<Vec<PendingRow>> {
    let conn = state
        .db
        .connect()
        .map_err(|error| anyhow::anyhow!("push_drain: connect failed: {error}"))?;
    let mut rows = conn
        .query(
            "SELECT i.id, i.device_id, i.content_type, i.body, i.attempts, d.direction \
             FROM device_push_inbox i \
             LEFT JOIN devices d ON d.id = i.device_id \
             WHERE i.status = 'pending' \
             ORDER BY i.received_at ASC \
             LIMIT ?1",
            params![limit],
        )
        .await
        .map_err(|error| anyhow::anyhow!("push_drain: select pending failed: {error}"))?;

    let mut out = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| anyhow::anyhow!("push_drain: row read failed: {error}"))?
    {
        out.push(PendingRow {
            id: row.get(0)?,
            device_id: row.get(1)?,
            content_type: row.get(2)?,
            body: row.get(3)?,
            attempts: row.get(4)?,
            direction_default: row.get(5)?,
        });
    }
    Ok(out)
}

fn truncate_error(reason: &str) -> String {
    if reason.chars().count() <= MAX_ERROR_LEN {
        reason.to_string()
    } else {
        let truncated: String = reason.chars().take(MAX_ERROR_LEN).collect();
        format!("{truncated}… [truncated]")
    }
}

async fn mark_done(state: &AppState, id: &str) -> Result<(), crate::db::write_queue::DbWriteError> {
    state
        .db_write
        .background_statement(
            "push_drain.mark-done",
            "UPDATE device_push_inbox SET status = 'done', processed_at = unixepoch() \
             WHERE id = ?1",
            vec![libsql::Value::Text(id.to_string())],
        )
        .await
        .map(|_| ())
}

async fn mark_failed(
    state: &AppState,
    id: &str,
    attempts: i64,
    reason: &str,
) -> Result<(), crate::db::write_queue::DbWriteError> {
    state
        .db_write
        .background_statement(
            "push_drain.mark-failed",
            "UPDATE device_push_inbox \
             SET status = 'failed', attempts = ?2, last_error = ?3, processed_at = unixepoch() \
             WHERE id = ?1",
            vec![
                libsql::Value::Text(id.to_string()),
                libsql::Value::Integer(attempts),
                libsql::Value::Text(truncate_error(reason)),
            ],
        )
        .await
        .map(|_| ())
}

async fn mark_retry(
    state: &AppState,
    id: &str,
    attempts: i64,
    reason: &str,
) -> Result<(), crate::db::write_queue::DbWriteError> {
    state
        .db_write
        .background_statement(
            "push_drain.mark-retry",
            "UPDATE device_push_inbox SET attempts = ?2, last_error = ?3 WHERE id = ?1",
            vec![
                libsql::Value::Text(id.to_string()),
                libsql::Value::Integer(attempts),
                libsql::Value::Text(truncate_error(reason)),
            ],
        )
        .await
        .map(|_| ())
}

/// Periodic owner. Mirrors `capture_cleanup::run`: a fixed-cadence interval,
/// cancelled via the same `CancellationToken` main.rs wires into every other
/// worker, with a final drain pass on shutdown so a burst of pushes right
/// before a restart does not sit untouched until the next startup.
pub async fn run(state: AppState, shutdown: CancellationToken) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(DRAIN_CADENCE);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                drain_once(&state).await?;
                return Ok(());
            }
            _ = interval.tick() => {
                drain_once(&state).await?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_error_passes_short_text_through_unchanged() {
        assert_eq!(truncate_error("database busy"), "database busy");
    }

    #[test]
    fn truncate_error_caps_a_long_chain_and_marks_it() {
        let long = "x".repeat(MAX_ERROR_LEN + 500);
        let truncated = truncate_error(&long);
        assert!(truncated.ends_with("… [truncated]"));
        // The cap is on character count, not byte count; the marker text adds
        // to that, which is fine — this asserts the payload was actually cut,
        // not an exact byte length.
        assert!(truncated.chars().count() < long.chars().count());
    }
}
