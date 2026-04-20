//! DB-writing helpers used by the stream consumer and supervisor watchdog.
//!
//! Writers are intentionally disjoint:
//! - per-device stream tasks only transition `connection_state` → `online` and
//!   bump `last_seen_at` via `touch_last_seen`.
//! - the watchdog only transitions `connection_state` → `offline` when
//!   `last_seen_at < unixepoch() - 90`.
//! This keeps the state machine single-writer per transition and avoids
//! flapping.

use anyhow::Result;

use crate::state::AppState;

/// Set `devices.connection_state` to the given value and bump `updated_at`.
/// Used by the supervisor/stream lifecycle on `online`/`offline` transitions
/// and by the watchdog on stale-device detection.
pub async fn update_connection_state(
    state: &AppState,
    device_id: &str,
    new_state: &str,
) -> Result<()> {
    let conn = state.db.connect()?;
    conn.execute(
        "UPDATE devices SET connection_state = ?1, updated_at = unixepoch() WHERE id = ?2",
        libsql::params![new_state.to_string(), device_id.to_string()],
    )
    .await?;
    Ok(())
}

/// Refresh `devices.last_seen_at` to the current epoch. Called on every
/// alertStream heartbeat AND every real event (the first byte of a new
/// part is sufficient — the device is clearly reachable).
pub async fn touch_last_seen(state: &AppState, device_id: &str) -> Result<()> {
    let conn = state.db.connect()?;
    conn.execute(
        "UPDATE devices SET last_seen_at = unixepoch() WHERE id = ?1",
        libsql::params![device_id.to_string()],
    )
    .await?;
    Ok(())
}
