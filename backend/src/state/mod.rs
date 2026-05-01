use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::Config;
use crate::db::write_queue::DbWriteQueue;
use crate::enrollments::handlers::CapturesMap;
use crate::recompute::RecomputeRequest;
use crate::supervisor::LifecycleTx;

mod paths;
pub use paths::Paths;

/// SSE payload broadcast to all connected /events/stream clients.
/// Enriched with employee_name and department at broadcast time when available.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AttendanceEventSSEPayload {
    pub id: String,
    pub employee_id: Option<String>,
    pub employee_name: Option<String>,
    pub department: Option<String>,
    pub captured_at: String,
    pub direction: String,
    pub has_photo: bool,
}

/// Shared application state passed to all Axum handlers via State extractor.
/// Arc-wrapped fields allow cheap cloning across handler tasks.
///
/// `lifecycle_tx` is `Option<...>` because Phase 1 / 02-01 / 02-02 integration
/// tests build the router without a running supervisor. Handlers use
/// `.as_ref().map(|tx| tx.send(...))` so a None channel silently skips the
/// lifecycle signal — the supervisor, if ever started, will reconcile from
/// the DB anyway.
///
/// `recompute_tx` follows the same Option pattern: Phase 3's RecomputeWorker
/// consumes on the other side, but earlier-phase integration tests build
/// AppState without one. Event ingestion publishes via this sender only when
/// Some(_); None silently skips (worker reconciles via the nightly job).
///
/// `event_broadcast` is `Option<...>` for the same reason: integration tests
/// build AppState without a broadcast channel. Handlers and service code use
/// `.as_ref().map(|tx| tx.send(...))` so a None silently skips — no SSE
/// clients means no subscribers, so non-fatal.
///
/// `purge_tx` and `backfill_tx` (Phase 7): Option for the same reason as above.
/// Both are None in tests that do not spin up workers; handlers skip silently.
///
/// `captures`: Phase 7 kiosk-capture in-memory state (D-02 LOCKED).
/// Always Some — initialised at startup via `enrollments::handlers::new_captures_map()`.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<libsql::Database>,
    pub config: Arc<Config>,
    /// Phase 8 (08-01): filesystem roots for evidence, event JPEGs, enrollment
    /// photos, kiosk captures tmp, and override evidence. Injected via
    /// `Paths::from_env()` at startup; overridden in tests with a per-test
    /// `TempDir` via `Paths::for_test`. Reading from env at use-site (the old
    /// `leaves_root()` / `events_root()` helpers) was cwd-dependent and
    /// process-globally racy — see CLAUDE.md Conventions § Filesystem-root
    /// injection.
    pub paths: Arc<Paths>,
    /// Single-writer queue for SQLite/libSQL mutations.
    pub db_write: DbWriteQueue,
    pub lifecycle_tx: Option<LifecycleTx>,
    pub recompute_tx: Option<UnboundedSender<RecomputeRequest>>,
    pub event_broadcast: Option<broadcast::Sender<AttendanceEventSSEPayload>>,
    /// Phase 6: license gate flag. `Arc<AtomicBool>` so middleware reads
    /// branch-free without a lock. Set true at startup if cached JWT
    /// validates (see license::service::load_and_validate_license);
    /// flipped to true on successful activation. Stays true after JWT
    /// `exp` per D-07 soft expiry.
    pub license_valid: Arc<std::sync::atomic::AtomicBool>,
    /// Phase 7: purge worker channel (D-15). None in test setups.
    pub purge_tx: Option<UnboundedSender<crate::workers::purge::PurgeRequest>>,
    /// Phase 7: backfill worker channel (D-16). None in test setups.
    pub backfill_tx: Option<UnboundedSender<crate::workers::backfill::BackfillRequest>>,
    /// Phase 7: in-memory kiosk-capture session state (D-02).
    /// Shared across capture_from_device + get_capture handlers.
    pub captures: CapturesMap,
}
