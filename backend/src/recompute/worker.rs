//! mpsc-driven recompute worker with 500ms debounce + HashSet dedup.
//!
//! Mirrors the Phase 2 `Supervisor` pattern: biased `tokio::select!` between
//! the shutdown cancellation token and the request receiver. On first receive,
//! the worker drains the channel, waits 500ms, drains again — collapsing
//! event bursts (e.g. multi-device punch storm) down to a single recompute
//! per (employee_id, anchor_date). Errors are logged, not propagated.

use std::collections::HashSet;

use chrono::NaiveDate;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::daily_records::service as dr_service;
use crate::state::AppState;

use super::RecomputeRequest;

pub struct RecomputeWorker {
    state: AppState,
    shutdown: CancellationToken,
}

impl RecomputeWorker {
    pub fn new(state: AppState, shutdown: CancellationToken) -> Self {
        Self { state, shutdown }
    }

    pub async fn run(self, mut rx: mpsc::UnboundedReceiver<RecomputeRequest>) {
        let debounce = tokio::time::Duration::from_millis(500);
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    tracing::info!("recompute worker shutdown");
                    break;
                }
                maybe_req = rx.recv() => {
                    let Some(req) = maybe_req else {
                        tracing::info!("recompute channel closed, worker exiting");
                        break;
                    };
                    let mut pending: HashSet<(String, NaiveDate)> = HashSet::new();
                    pending.insert((req.employee_id, req.anchor_date));
                    // Drain anything already queued.
                    while let Ok(extra) = rx.try_recv() {
                        pending.insert((extra.employee_id, extra.anchor_date));
                    }
                    // Debounce window — let bursts collapse.
                    tokio::time::sleep(debounce).await;
                    while let Ok(extra) = rx.try_recv() {
                        pending.insert((extra.employee_id, extra.anchor_date));
                    }
                    for (emp_id, date) in pending.drain() {
                        if let Err(e) = dr_service::recompute_for_day(&self.state, &emp_id, date).await {
                            tracing::warn!(
                                employee_id = %emp_id,
                                anchor_date = %date,
                                err = %e,
                                "recompute failed"
                            );
                        }
                    }
                }
            }
        }
    }
}
