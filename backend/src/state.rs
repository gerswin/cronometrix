use std::sync::Arc;

use crate::config::Config;
use crate::supervisor::LifecycleTx;

/// Shared application state passed to all Axum handlers via State extractor.
/// Arc-wrapped fields allow cheap cloning across handler tasks.
///
/// `lifecycle_tx` is `Option<...>` because Phase 1 / 02-01 / 02-02 integration
/// tests build the router without a running supervisor. Handlers use
/// `.as_ref().map(|tx| tx.send(...))` so a None channel silently skips the
/// lifecycle signal — the supervisor, if ever started, will reconcile from
/// the DB anyway.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<libsql::Database>,
    pub config: Arc<Config>,
    pub lifecycle_tx: Option<LifecycleTx>,
}
