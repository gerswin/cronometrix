//! mpsc-driven recompute worker with 500ms debounce + HashSet dedup.
//!
//! Mirrors the Phase 2 `Supervisor` pattern: biased `tokio::select!` between
//! the shutdown cancellation token and the request receiver. On first receive,
//! the worker drains the channel, waits 500ms, drains again — collapsing
//! event bursts (e.g. multi-device punch storm) down to a single recompute
//! per (employee_id, anchor_date). Errors are logged, not propagated.

use std::collections::HashSet;

use anyhow::Context;
use chrono::NaiveDate;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::daily_records::service as dr_service;
use crate::db::write_queue::{DbWriteError, DbWriteQueue};
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
                    let mut drained = 0_u64;
                    while let Ok(request) = rx.try_recv() {
                        self.process_request(request).await;
                        drained += 1;
                    }
                    tracing::info!(drained, "recompute worker shutdown drained pending work");
                    break;
                }
                maybe_req = rx.recv() => {
                    let Some(req) = maybe_req else {
                        tracing::info!("recompute channel closed, worker exiting");
                        break;
                    };
                    let mut pending: HashSet<(String, NaiveDate)> = HashSet::new();
                    let first_range = collect_day_or_range(req, &mut pending);
                    // Debounce window — let bursts collapse.
                    tokio::time::sleep(debounce).await;
                    if let Some(range) = first_range {
                        self.process_request(range).await;
                    }
                    while let Ok(extra) = rx.try_recv() {
                        match extra {
                            RecomputeRequest::Day { employee_id, anchor_date } => {
                                pending.insert((employee_id, anchor_date));
                            }
                            range @ RecomputeRequest::Range { .. } => {
                                self.process_request(range).await;
                            }
                        }
                    }
                    for (emp_id, date) in pending.drain() {
                        self.process_day(&emp_id, date).await;
                    }
                }
            }
        }
    }

    async fn process_request(&self, request: RecomputeRequest) {
        match request {
            RecomputeRequest::Day {
                employee_id,
                anchor_date,
            } => self.process_day(&employee_id, anchor_date).await,
            RecomputeRequest::Range {
                employee_id,
                from_date,
                to_date,
            } => {
                let mut date = from_date;
                let mut processed = 0_u64;
                while date <= to_date {
                    self.process_day(&employee_id, date).await;
                    processed += 1;
                    if processed % 64 == 0 {
                        tokio::task::yield_now().await;
                    }
                    let Some(next) = date.succ_opt() else {
                        break;
                    };
                    date = next;
                }
            }
        }
    }

    async fn process_day(&self, employee_id: &str, anchor_date: NaiveDate) {
        if let Err(error) =
            dr_service::recompute_for_day(&self.state, employee_id, anchor_date).await
        {
            tracing::warn!(err = %error, "recompute failed; identifiers omitted");
        }
    }
}

fn collect_day_or_range(
    request: RecomputeRequest,
    pending: &mut HashSet<(String, NaiveDate)>,
) -> Option<RecomputeRequest> {
    match request {
        RecomputeRequest::Day {
            employee_id,
            anchor_date,
        } => {
            pending.insert((employee_id, anchor_date));
            None
        }
        range @ RecomputeRequest::Range { .. } => Some(range),
    }
}

/// Complete producer-first shutdown. HTTP/background producers must already be
/// stopped. The first flush commits every previously accepted mutation and its
/// post-commit callbacks while recompute is alive. Recompute is then cancelled,
/// drains its pending units (and awaits their derived DB writes), after which
/// the database writer can reject new admission and drain to completion.
pub async fn shutdown_after_producers(
    db_write: DbWriteQueue,
    recompute_shutdown: CancellationToken,
    recompute_handle: JoinHandle<()>,
    db_write_handle: JoinHandle<Result<(), DbWriteError>>,
) -> anyhow::Result<()> {
    db_write
        .flush()
        .await
        .context("flush accepted writes before recompute shutdown")?;
    recompute_shutdown.cancel();
    let recompute_result = recompute_handle
        .await
        .context("join recompute worker during shutdown");
    let close_result = db_write
        .close_and_flush()
        .await
        .context("close database writer after recompute drain");
    let writer_result = db_write_handle
        .await
        .context("join database writer during shutdown")?
        .map_err(anyhow::Error::from);
    recompute_result?;
    close_result?;
    writer_result
}
