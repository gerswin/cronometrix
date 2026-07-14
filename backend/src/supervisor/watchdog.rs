//! Stale-device watchdog (Plan 02-03 Task 2).
//!
//! Runs a single UPDATE every 10 seconds that flips any active device whose
//! `last_seen_at` is older than 90 seconds to `connection_state = 'offline'`.
//! Only the watchdog performs this transition — per-device stream tasks
//! handle the reverse direction. This makes the state machine single-writer
//! per transition and avoids oscillation.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::state::AppState;

/// How often the watchdog runs. Exposed for test determinism (tests call
/// `run_once` directly and never wait for the tick).
pub const WATCHDOG_INTERVAL_SECS: u64 = 10;

/// How stale `last_seen_at` must be before we mark the device offline.
pub const STALE_THRESHOLD_SECS: i64 = 90;

/// Long-running watchdog loop. Exits on CancellationToken.
pub async fn watchdog_task(state: AppState, cancel: CancellationToken) {
    let mut interval = tokio::time::interval(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
    // The first `tick` fires immediately; skip it so we don't double-hammer
    // the DB at startup before the supervisor finishes bootstrapping.
    interval.tick().await;
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                tracing::info!("watchdog cancelled");
                return;
            }
            _ = interval.tick() => {
                if let Err(e) = run_once(&state).await {
                    tracing::warn!(err = %e, "watchdog iteration failed");
                }
            }
        }
    }
}

/// Single iteration of the watchdog sweep. Public so integration tests can
/// invoke it deterministically without waiting for the interval tick.
pub async fn run_once(state: &AppState) -> anyhow::Result<u64> {
    let rows = state
        .db_write
        .background_statement(
            "supervisor.watchdog-offline",
            "UPDATE devices \
             SET connection_state = 'offline', updated_at = unixepoch() \
             WHERE status = 'active' \
               AND connection_state != 'offline' \
               AND (last_seen_at IS NULL OR last_seen_at < unixepoch() - ?1)",
            vec![libsql::Value::Integer(STALE_THRESHOLD_SECS)],
        )
        .await?;
    Ok(rows)
}
