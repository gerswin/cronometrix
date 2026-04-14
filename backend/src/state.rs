use std::sync::Arc;

use crate::config::Config;

/// Shared application state passed to all Axum handlers via State extractor.
/// Arc-wrapped fields allow cheap cloning across handler tasks.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<libsql::Database>,
    pub config: Arc<Config>,
}
