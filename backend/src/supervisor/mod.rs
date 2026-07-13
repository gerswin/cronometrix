//! Per-device alertStream supervisor (Plan 02-03).
//!
//! Owns one tokio task per active device, each running an exponential-backoff
//! reconnect loop against the device's alertStream endpoint. Accepts
//! lifecycle events via an mpsc channel from the devices CRUD handlers so
//! edits/deletes reconcile without a process restart (Pitfall 7).
//!
//! Task 1 of 02-03 wires up the `status` helpers used by `isapi::stream`.
//! Task 2 populates the full `Supervisor` + `task` + `watchdog` modules.

pub mod status;
pub mod task;
pub mod watchdog;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::devices::models::DeviceWithPlaintext;
use crate::devices::service as devices_service;
use crate::state::AppState;

/// Lifecycle signal emitted by the `devices` CRUD handlers.
///
/// `Start`    — brand-new active device, load from DB and spawn a task.
/// `Restart`  — connection-affecting field changed (ip/port/scheme/user/pass/
///              allow_insecure_tls/status). Stop the existing task (if any),
///              reload from DB, respawn.
/// `Stop`     — device deactivated / deleted. Cancel the task and drop it.
#[derive(Debug, Clone)]
pub enum DeviceLifecycleEvent {
    Start(String),
    Stop(String),
    Restart(String),
}

/// Sender half of the lifecycle mpsc channel. Lives inside `AppState`.
pub type LifecycleTx = mpsc::UnboundedSender<DeviceLifecycleEvent>;

/// Long-lived supervisor that owns one child task per active device.
///
/// Internals (Mutex-protected handle map) are implementation detail; the
/// outer lifecycle API is the `DeviceLifecycleEvent` channel.
pub struct Supervisor {
    state: AppState,
    shutdown: CancellationToken,
    handles: Arc<Mutex<HashMap<String, (JoinHandle<()>, CancellationToken)>>>,
}

impl Supervisor {
    pub fn new(state: AppState, shutdown: CancellationToken) -> Self {
        Self {
            state,
            shutdown,
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Main supervisor loop. Bootstraps from the `devices` table, then reads
    /// lifecycle events off `lifecycle_rx` until the shutdown token fires.
    ///
    /// On shutdown, cancels every child task and awaits their joins so no
    /// reqwest stream leaks past process exit.
    pub async fn run(self, mut lifecycle_rx: mpsc::UnboundedReceiver<DeviceLifecycleEvent>) {
        // Bootstrap: spawn a task for every currently-active device.
        match self.state.db.connect() {
            Ok(conn) => {
                match devices_service::list_active(&conn, &self.state.config.device_creds_key).await
                {
                    Ok(devices) => {
                        tracing::info!(count = devices.len(), "supervisor bootstrapping");
                        for dev in devices {
                            self.spawn_device(dev).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!(err = %e, "supervisor failed to list active devices on bootstrap");
                    }
                }
            }
            Err(e) => {
                tracing::error!(err = %e, "supervisor failed to acquire DB connection on bootstrap");
            }
        }

        // Reconcile loop.
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => {
                    tracing::info!("supervisor received shutdown");
                    break;
                }
                Some(ev) = lifecycle_rx.recv() => {
                    self.handle_event(ev).await;
                }
            }
        }

        // Graceful shutdown: drain handles map, cancel every child, await all.
        let drained: Vec<(String, (JoinHandle<()>, CancellationToken))> = {
            let mut h = self.handles.lock().await;
            std::mem::take(&mut *h).into_iter().collect()
        };
        for (_id, (_h, tok)) in &drained {
            tok.cancel();
        }
        for (id, (handle, _tok)) in drained {
            if let Err(e) = handle.await {
                tracing::warn!(device_id = %id, err = ?e, "device task join error on shutdown");
            }
        }
        tracing::info!("supervisor shutdown complete");
    }

    async fn spawn_device(&self, dev: DeviceWithPlaintext) {
        let dev_id = dev.id.clone();
        // If already running, do nothing. Defensive — handle_event's Start arm
        // should have caught this, but we guard against double-spawn.
        {
            let h = self.handles.lock().await;
            if h.contains_key(&dev_id) {
                tracing::debug!(device_id = %dev_id, "device task already running, skip spawn");
                return;
            }
        }
        let child_tok = self.shutdown.child_token();
        let state = self.state.clone();
        let tok_clone = child_tok.clone();
        let handle = tokio::spawn(async move {
            task::device_task(dev, state, tok_clone).await;
        });
        self.handles
            .lock()
            .await
            .insert(dev_id, (handle, child_tok));
    }

    async fn stop_device(&self, id: &str) {
        let removed = {
            let mut h = self.handles.lock().await;
            h.remove(id)
        };
        if let Some((handle, tok)) = removed {
            tok.cancel();
            if let Err(e) = handle.await {
                tracing::warn!(device_id = %id, err = ?e, "device task join error on stop");
            }
        }
    }

    async fn handle_event(&self, ev: DeviceLifecycleEvent) {
        match ev {
            DeviceLifecycleEvent::Start(id) => {
                // Re-read from DB (the handler has already committed).
                match self.state.db.connect() {
                    Ok(conn) => match devices_service::get_decrypted(
                        &conn,
                        &id,
                        &self.state.config.device_creds_key,
                    )
                    .await
                    {
                        Ok(dev) => self.spawn_device(dev).await,
                        Err(e) => {
                            tracing::warn!(device_id = %id, err = %e, "Start: device not loadable, skipping spawn")
                        }
                    },
                    Err(e) => tracing::error!(err = %e, "Start: DB connect failed"),
                }
            }
            DeviceLifecycleEvent::Stop(id) => {
                self.stop_device(&id).await;
            }
            DeviceLifecycleEvent::Restart(id) => {
                self.stop_device(&id).await;
                match self.state.db.connect() {
                    Ok(conn) => match devices_service::get_decrypted(
                        &conn,
                        &id,
                        &self.state.config.device_creds_key,
                    )
                    .await
                    {
                        Ok(dev) => self.spawn_device(dev).await,
                        Err(e) => {
                            tracing::warn!(device_id = %id, err = %e, "Restart: device not loadable, not respawning")
                        }
                    },
                    Err(e) => tracing::error!(err = %e, "Restart: DB connect failed"),
                }
            }
        }
    }

    /// Test-only accessor: number of active per-device tasks currently tracked.
    #[cfg(test)]
    pub async fn active_count(&self) -> usize {
        self.handles.lock().await.len()
    }

    /// Test-only: snapshot the set of tracked device ids.
    #[cfg(test)]
    pub async fn active_ids(&self) -> Vec<String> {
        let h = self.handles.lock().await;
        h.keys().cloned().collect()
    }
}
