//! PurgeWorker — delete all device face mappings for a deactivated employee (D-15).
//!
//! Driven by mpsc channel. Uses biased select to drain shutdown first.
//! Applies Pitfall 10 guard: re-reads employee status mid-loop before each
//! device delete — aborts the entire batch if the employee was re-activated.

use std::collections::HashSet;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

use crate::devices::service as devices_service;
use crate::enrollments::service as enrollment_service;
use crate::isapi::client::DeviceConnection;
use crate::state::AppState;

/// Request to purge all device face mappings for a deactivated employee (D-15).
#[derive(Debug, Clone)]
pub struct PurgeRequest {
    pub employee_id: String,
}

/// Worker that processes PurgeRequests from a channel.
///
/// Lifecycle:
///   1. Receive batch of requests (may deduplicate if requests arrive faster than processing).
///   2. For each unique employee_id:
///      a. Re-read employee status (Pitfall 10 — abort if re-activated).
///      b. Fetch all device_face_mappings rows.
///      c. For each row: fetch DeviceWithPlaintext, call delete_user with 30s timeout.
///         On Ok → DELETE mapping row.
///         On Err → UPDATE state='pending_delete' (retry on next purge trigger).
pub struct PurgeWorker {
    state: AppState,
    shutdown: CancellationToken,
}

impl PurgeWorker {
    pub fn new(state: AppState, shutdown: CancellationToken) -> Self {
        Self { state, shutdown }
    }

    pub async fn run(self, mut rx: UnboundedReceiver<PurgeRequest>) {
        tracing::info!("PurgeWorker started");

        loop {
            // Biased select: shutdown wins if both ready.
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    tracing::info!("PurgeWorker shutting down");
                    return;
                }
                msg = rx.recv() => {
                    match msg {
                        None => {
                            tracing::info!("PurgeWorker: channel closed, exiting");
                            return;
                        }
                        Some(req) => {
                            // Drain all immediately available requests for batching / dedup.
                            let mut ids: HashSet<String> = HashSet::new();
                            ids.insert(req.employee_id);

                            while let Ok(extra) = rx.try_recv() {
                                ids.insert(extra.employee_id);
                            }

                            for employee_id in ids {
                                self.process_employee(&employee_id).await;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn process_employee(&self, employee_id: &str) {
        let conn = match self.state.db.connect() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(employee_id = %employee_id, err = %e, "PurgeWorker: db connect failed");
                return;
            }
        };

        // Pitfall 10: re-read employee status before acting.
        let status = match enrollment_service::get_employee_status(&conn, employee_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(employee_id = %employee_id, err = %e, "PurgeWorker: failed to read employee status");
                return;
            }
        };

        if status != "inactive" {
            tracing::info!(
                employee_id = %employee_id,
                status = %status,
                "PurgeWorker: employee re-activated or not inactive — skipping purge"
            );
            return;
        }

        // Fetch all active/pending_delete mappings: (mapping_id, device_id, face_id).
        let mappings = match enrollment_service::list_mappings_for_employee(&conn, employee_id).await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(employee_id = %employee_id, err = %e, "PurgeWorker: failed to list mappings");
                return;
            }
        };

        if mappings.is_empty() {
            tracing::info!(employee_id = %employee_id, "PurgeWorker: no mappings to purge");
            return;
        }

        tracing::info!(
            employee_id = %employee_id,
            mapping_count = mappings.len(),
            "PurgeWorker: purging face mappings"
        );

        for (mapping_id, device_id, face_id) in mappings {
            // Pitfall 10: re-read employee status before each device delete.
            let current_status = match enrollment_service::get_employee_status(&conn, employee_id).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(employee_id = %employee_id, err = %e, "PurgeWorker: status re-read failed mid-loop");
                    break;
                }
            };

            if current_status != "inactive" {
                tracing::info!(
                    employee_id = %employee_id,
                    "PurgeWorker: employee re-activated mid-loop — aborting purge batch"
                );
                break;
            }

            // Fetch device with decrypted credentials.
            let device = match devices_service::get_decrypted(
                &conn,
                &device_id,
                &self.state.config.device_creds_key,
            )
            .await
            {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!(device_id = %device_id, err = %e, "PurgeWorker: failed to fetch device, marking pending_delete");
                    if let Err(ue) = enrollment_service::mark_mapping_pending_delete_queued(&self.state, &mapping_id).await {
                        tracing::error!(err = %ue, "PurgeWorker: failed to mark pending_delete");
                    }
                    continue;
                }
            };

            let isapi = match DeviceConnection::new(
                &device.base_url,
                &device.username,
                &device.password,
                device.allow_insecure_tls,
            ) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(device_id = %device_id, err = %e, "PurgeWorker: failed to build DeviceConnection");
                    if let Err(ue) = enrollment_service::mark_mapping_pending_delete_queued(&self.state, &mapping_id).await {
                        tracing::error!(err = %ue, "PurgeWorker: failed to mark pending_delete");
                    }
                    continue;
                }
            };

            let fid = face_id.clone();
            let result = tokio::time::timeout(
                Duration::from_secs(30),
                async move { isapi.delete_user(&fid).await },
            )
            .await;

            match result {
                Ok(Ok(_)) => {
                    if let Err(e) = enrollment_service::delete_mapping_queued(&self.state, &mapping_id).await {
                        tracing::error!(mapping_id = %mapping_id, err = %e, "PurgeWorker: failed to delete mapping row");
                    } else {
                        tracing::info!(
                            employee_id = %employee_id,
                            device_id = %device_id,
                            face_id = %face_id,
                            "PurgeWorker: face deleted from device"
                        );
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!(device_id = %device_id, err = %e, "PurgeWorker: delete_user failed");
                    if let Err(ue) = enrollment_service::mark_mapping_pending_delete_queued(&self.state, &mapping_id).await {
                        tracing::error!(err = %ue, "PurgeWorker: failed to mark pending_delete");
                    }
                }
                Err(_timeout) => {
                    tracing::warn!(device_id = %device_id, "PurgeWorker: delete_user timeout");
                    if let Err(ue) = enrollment_service::mark_mapping_pending_delete_queued(&self.state, &mapping_id).await {
                        tracing::error!(err = %ue, "PurgeWorker: failed to mark pending_delete after timeout");
                    }
                }
            }
        }
    }
}
