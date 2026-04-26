//! Reports service — Task 2 will populate this with the SQL aggregation, leaves
//! secondary aggregation (W-5), money math wiring, and audit insert.

use super::models::{ReportParamsRequest, ReportPayload};
use crate::{errors::AppError, state::AppState};

/// Stub: real implementation lands in Task 2.
pub async fn compute_report(
    _state: &AppState,
    _actor_id: &str,
    _params: &ReportParamsRequest,
    _format: &str,
) -> Result<ReportPayload, AppError> {
    Err(AppError::Internal(anyhow::anyhow!(
        "compute_report not yet implemented (Task 2)"
    )))
}
