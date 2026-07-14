//! Fair, bounded recompute scheduling plus producer-first shutdown.
//!
//! Leave ranges remain a constant-size request on the channel and are expanded
//! here in 32-day chunks. Continuations go to the back of the local queue, so
//! recent single-day work is not trapped behind a maximum-size leave.

use std::collections::{HashSet, VecDeque};

use anyhow::Context;
use chrono::NaiveDate;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::daily_records::service as dr_service;
use crate::db::write_queue::{DbWriteError, DbWriteQueue};
use crate::state::AppState;

use super::RecomputeRequest;

const RANGE_CHUNK_DAYS: usize = 32;
const RECEIVE_BURST_LIMIT: usize = 64;

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
        let mut ready = VecDeque::new();
        let mut pending_days = HashSet::new();
        let mut draining = false;
        let mut drained = 0_u64;

        loop {
            if self.shutdown.is_cancelled() {
                draining = true;
            }

            if let Some(request) = ready.pop_front() {
                if let RecomputeRequest::Day {
                    employee_id,
                    anchor_date,
                } = &request
                {
                    pending_days.remove(&(employee_id.clone(), *anchor_date));
                }

                let continuation = self.process_chunk(request).await;
                if draining {
                    drained += 1;
                }

                // Give requests which arrived during this chunk priority over
                // its continuation. The bounded burst keeps local scheduling
                // work predictable even when the channel is busy.
                drain_receiver(&mut rx, &mut ready, &mut pending_days, RECEIVE_BURST_LIMIT);
                if let Some(continuation) = continuation {
                    ready.push_back(continuation);
                }
                continue;
            }

            if draining {
                drain_receiver(&mut rx, &mut ready, &mut pending_days, RECEIVE_BURST_LIMIT);
                if ready.is_empty() {
                    tracing::info!(drained, "recompute worker shutdown drained pending work");
                    break;
                }
                continue;
            }

            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    draining = true;
                }
                maybe_req = rx.recv() => {
                    let Some(request) = maybe_req else {
                        tracing::info!("recompute channel closed, worker exiting");
                        break;
                    };
                    enqueue(request, &mut ready, &mut pending_days);

                    // Preserve burst collapsing for ordinary day requests.
                    tokio::select! {
                        biased;
                        _ = self.shutdown.cancelled() => {
                            draining = true;
                        }
                        _ = tokio::time::sleep(debounce) => {}
                    }
                    drain_receiver(
                        &mut rx,
                        &mut ready,
                        &mut pending_days,
                        RECEIVE_BURST_LIMIT,
                    );
                }
            }
        }
    }

    async fn process_chunk(&self, request: RecomputeRequest) -> Option<RecomputeRequest> {
        match request {
            RecomputeRequest::Day {
                employee_id,
                anchor_date,
            } => {
                self.process_day(&employee_id, anchor_date).await;
                None
            }
            RecomputeRequest::Range {
                employee_id,
                from_date,
                to_date,
            } => {
                let mut date = from_date;
                for _ in 0..RANGE_CHUNK_DAYS {
                    self.process_day(&employee_id, date).await;
                    if date >= to_date {
                        return None;
                    }
                    let next = date.succ_opt()?;
                    date = next;
                }

                Some(RecomputeRequest::Range {
                    employee_id,
                    from_date: date,
                    to_date,
                })
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

fn enqueue(
    request: RecomputeRequest,
    ready: &mut VecDeque<RecomputeRequest>,
    pending_days: &mut HashSet<(String, NaiveDate)>,
) {
    if let RecomputeRequest::Day {
        employee_id,
        anchor_date,
    } = &request
    {
        if !pending_days.insert((employee_id.clone(), *anchor_date)) {
            return;
        }
    }
    ready.push_back(request);
}

fn drain_receiver(
    rx: &mut mpsc::UnboundedReceiver<RecomputeRequest>,
    ready: &mut VecDeque<RecomputeRequest>,
    pending_days: &mut HashSet<(String, NaiveDate)>,
    limit: usize,
) {
    for _ in 0..limit {
        let Ok(request) = rx.try_recv() else {
            break;
        };
        enqueue(request, ready, pending_days);
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
    // Do not short-circuit cleanup. Preserve the initial flush error because it
    // is the earliest failure, while still deterministically joining workers.
    let flush_result = db_write
        .flush()
        .await
        .context("flush accepted writes before recompute shutdown");

    recompute_shutdown.cancel();
    let recompute_result = recompute_handle
        .await
        .context("join recompute worker during shutdown");
    let close_result = db_write
        .close_and_flush()
        .await
        .context("close database writer after recompute drain");
    let writer_result = match db_write_handle.await {
        Ok(result) => result.map_err(anyhow::Error::from),
        Err(error) => {
            Err(anyhow::Error::new(error).context("join database writer during shutdown"))
        }
    };

    flush_result?;
    recompute_result?;
    close_result?;
    writer_result
}
