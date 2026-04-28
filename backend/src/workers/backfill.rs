//! BackfillWorker — push all active employee face profiles to a newly registered device (D-16).
//!
//! Driven by mpsc channel. Caps concurrent per-employee pushes at 4 via Semaphore.
//! Uses JoinSet so the driver awaits all pushes before releasing the semaphore slots.

use std::sync::Arc;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::devices::service as devices_service;
use crate::enrollments::pusher::push_one_device_for_backfill;
use crate::enrollments::service as enrollment_service;
use crate::state::AppState;

/// Request to backfill all active employee face profiles to a newly registered device (D-16).
#[derive(Debug, Clone)]
pub struct BackfillRequest {
    pub device_id: String,
}

/// Worker that processes BackfillRequests from a channel.
///
/// For each request:
///   1. Fetch the target device with decrypted credentials.
///   2. Fetch all active employees that have a face_id enrolled.
///   3. Fan-out pushes with a concurrency cap of 4 (Semaphore::new(4)).
///   4. On success per employee: upsert device_face_mappings (handled inside push_one_device_for_backfill).
pub struct BackfillWorker {
    state: AppState,
    shutdown: CancellationToken,
}

impl BackfillWorker {
    pub fn new(state: AppState, shutdown: CancellationToken) -> Self {
        Self { state, shutdown }
    }

    pub async fn run(self, mut rx: UnboundedReceiver<BackfillRequest>) {
        tracing::info!("BackfillWorker started");

        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    tracing::info!("BackfillWorker shutting down");
                    return;
                }
                msg = rx.recv() => {
                    match msg {
                        None => {
                            tracing::info!("BackfillWorker: channel closed, exiting");
                            return;
                        }
                        Some(req) => {
                            self.process_device(&req.device_id).await;
                        }
                    }
                }
            }
        }
    }

    async fn process_device(&self, device_id: &str) {
        let conn = match self.state.db.connect() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(device_id = %device_id, err = %e, "BackfillWorker: db connect failed");
                return;
            }
        };

        // Fetch device with decrypted credentials.
        let device = match devices_service::get_decrypted(
            &conn,
            device_id,
            &self.state.config.device_creds_key,
        )
        .await
        {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(device_id = %device_id, err = %e, "BackfillWorker: failed to fetch device");
                return;
            }
        };

        // Fetch all active employees with an enrolled face: (employee_id, face_id, _cfe_id).
        let employees = match enrollment_service::list_employees_with_face(&conn).await {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(device_id = %device_id, err = %e, "BackfillWorker: failed to list employees with face");
                return;
            }
        };

        if employees.is_empty() {
            tracing::info!(device_id = %device_id, "BackfillWorker: no enrolled employees to backfill");
            return;
        }

        // Resolve name + photo_path for each employee.
        // (list_employees_with_face returns (employee_id, face_id, cfe_id) — query name/photo separately)
        let mut resolved: Vec<(String, String, String, String)> = Vec::new();
        for (employee_id, face_id, _cfe_id) in employees {
            // Employee name.
            let name = {
                let mut rows = match conn
                    .query(
                        "SELECT name FROM employees WHERE id = ?1",
                        libsql::params![employee_id.clone()],
                    )
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(employee_id = %employee_id, err = %e, "BackfillWorker: name query failed, using id as name");
                        // Use employee_id as fallback name rather than skipping.
                        resolved.push((employee_id.clone(), face_id, employee_id.clone(), String::new()));
                        continue;
                    }
                };
                match rows.next().await {
                    Ok(Some(row)) => row.get::<String>(0).unwrap_or_else(|_| employee_id.clone()),
                    _ => employee_id.clone(),
                }
            };

            // Current photo path.
            let photo_path = match enrollment_service::get_current_photo_path(&conn, &employee_id).await {
                Ok(Some(p)) => p,
                Ok(None) => {
                    tracing::warn!(employee_id = %employee_id, "BackfillWorker: no photo_path — skipping");
                    continue;
                }
                Err(e) => {
                    tracing::warn!(employee_id = %employee_id, err = %e, "BackfillWorker: photo_path query failed — skipping");
                    continue;
                }
            };

            resolved.push((employee_id, face_id, name, photo_path));
        }

        if resolved.is_empty() {
            tracing::info!(device_id = %device_id, "BackfillWorker: no employees with photo_path to backfill");
            return;
        }

        tracing::info!(
            device_id = %device_id,
            employee_count = resolved.len(),
            "BackfillWorker: starting backfill"
        );

        let sem = Arc::new(Semaphore::new(4));
        let mut set: JoinSet<()> = JoinSet::new();
        let device = Arc::new(device);

        for (employee_id, face_id, full_name, photo_path) in resolved {
            let state = self.state.clone();
            let device = Arc::clone(&device);
            let sem = Arc::clone(&sem);

            set.spawn(async move {
                // Acquire semaphore slot (cap at 4 concurrent pushes).
                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => {
                        tracing::warn!(employee_id = %employee_id, "BackfillWorker: semaphore closed");
                        return;
                    }
                };

                // Read the photo from disk.
                let photo_bytes = match tokio::fs::read(&photo_path).await {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(
                            employee_id = %employee_id,
                            photo_path = %photo_path,
                            err = %e,
                            "BackfillWorker: failed to read photo"
                        );
                        return;
                    }
                };

                match push_one_device_for_backfill(
                    &state,
                    &face_id,
                    &photo_bytes,
                    &employee_id,
                    &full_name,
                    &device,
                )
                .await
                {
                    Ok(_) => {
                        tracing::info!(
                            employee_id = %employee_id,
                            device_id = %device.id,
                            "BackfillWorker: face pushed successfully"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            employee_id = %employee_id,
                            device_id = %device.id,
                            err = %e,
                            "BackfillWorker: face push failed"
                        );
                    }
                }
            });
        }

        // Drain all push results.
        while let Some(res) = set.join_next().await {
            if let Err(e) = res {
                tracing::error!(err = %e, "BackfillWorker: push task panicked");
            }
        }

        tracing::info!(device_id = %device_id, "BackfillWorker: backfill complete");
    }
}
