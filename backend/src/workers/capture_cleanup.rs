//! Lifecycle owner for kiosk-capture state and temporary JPEGs.
//!
//! SIGKILL cannot execute compensation. The startup orphan sweep is the
//! recovery path for files left behind by an ungraceful process death.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use tokio_util::sync::CancellationToken;

use crate::state::AppState;
use crate::storage::atomic_file::remove_owned_file;

pub const CAPTURING_TTL: Duration = Duration::from_secs(45);
pub const TERMINAL_TTL: Duration = Duration::from_secs(5 * 60);
pub const CLEANUP_CADENCE: Duration = Duration::from_secs(30);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CleanupStats {
    pub timed_out: usize,
    pub states_removed: usize,
    pub jpegs_removed: usize,
    pub delete_failures: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct CleanupNow {
    monotonic: Instant,
    wall: SystemTime,
}

impl CleanupNow {
    pub fn new(monotonic: Instant, wall: SystemTime) -> Self {
        Self { monotonic, wall }
    }

    pub fn now() -> Self {
        Self::new(Instant::now(), SystemTime::now())
    }
}

pub async fn cleanup_once(state: &AppState, now: CleanupNow) -> anyhow::Result<CleanupStats> {
    cleanup_once_with_after_delete(state, now, |_| std::future::ready(())).await
}

/// Variant used by deterministic race tests to interleave a state replacement
/// after file deletion but before compare-and-remove.
#[doc(hidden)]
pub async fn cleanup_once_with_after_delete<F, Fut>(
    state: &AppState,
    now: CleanupNow,
    mut after_delete: F,
) -> anyhow::Result<CleanupStats>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root).await?;
    let mut stats = CleanupStats::default();
    cleanup_active_states(state, now.monotonic, &mut stats, &mut after_delete).await?;
    sweep_orphan_jpegs(state, now.wall, &mut stats).await?;
    Ok(stats)
}

async fn cleanup_active_states<F, Fut>(
    state: &AppState,
    now: Instant,
    stats: &mut CleanupStats,
    after_delete: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    {
        let mut entries = state.captures.write().await;
        for capture in entries.values_mut() {
            if capture.status == "capturing"
                && now.saturating_duration_since(capture.created_at) >= CAPTURING_TTL
            {
                capture.status = "timeout".into();
                capture.error_message =
                    Some("Capture exceeded the 45 second lifecycle deadline".into());
                capture.terminal_at = Some(now);
                stats.timed_out += 1;
            }
        }
    }

    let expired: Vec<(String, String, Option<PathBuf>, Instant)> = {
        let entries = state.captures.read().await;
        entries
            .iter()
            .filter_map(|(id, capture)| {
                let terminal_at = capture.terminal_at?;
                (now.saturating_duration_since(terminal_at) >= TERMINAL_TTL).then(|| {
                    (
                        id.clone(),
                        capture.status.clone(),
                        capture.photo_path.as_deref().map(PathBuf::from),
                        terminal_at,
                    )
                })
            })
            .collect()
    };

    for (id, status, photo_path, terminal_at) in expired {
        let unchanged = state.captures.read().await.get(&id).is_some_and(|capture| {
            capture.status == status && capture.terminal_at == Some(terminal_at)
        });
        if !unchanged {
            continue;
        }
        let mut jpeg_removed = false;
        if status == "captured" {
            let Some(path) = photo_path else {
                warn_delete_failure(&id, "missing-photo-path");
                stats.delete_failures += 1;
                continue;
            };
            if !is_direct_jpeg_child(&state.paths.captures_tmp_root, &path) {
                warn_delete_failure(&id, "outside-capture-root");
                stats.delete_failures += 1;
                continue;
            }
            match remove_owned_file(&state.paths.captures_tmp_root, &path) {
                Ok(()) => {
                    stats.jpegs_removed += 1;
                    jpeg_removed = true;
                    after_delete(id.clone()).await;
                }
                Err(error) => {
                    warn_delete_failure(&id, anyhow_error_kind(&error));
                    stats.delete_failures += 1;
                    continue;
                }
            }
        }
        if status != "captured" || jpeg_removed {
            let mut entries = state.captures.write().await;
            let unchanged = entries.get(&id).is_some_and(|capture| {
                capture.status == status && capture.terminal_at == Some(terminal_at)
            });
            if unchanged {
                entries.remove(&id);
                stats.states_removed += 1;
            }
        }
    }
    Ok(())
}

async fn sweep_orphan_jpegs(
    state: &AppState,
    wall_now: SystemTime,
    stats: &mut CleanupStats,
) -> anyhow::Result<()> {
    let owned: HashSet<String> = {
        let entries = state.captures.read().await;
        entries
            .iter()
            .flat_map(|(id, capture)| {
                let mut names = vec![format!("{id}.jpg")];
                if let Some(name) = capture
                    .photo_path
                    .as_deref()
                    .and_then(|path| Path::new(path).file_name())
                    .and_then(|name| name.to_str())
                {
                    names.push(name.to_string());
                }
                names
            })
            .collect()
    };

    let mut directory = tokio::fs::read_dir(&state.paths.captures_tmp_root).await?;
    while let Some(entry) = directory.next_entry().await? {
        let file_type = entry.file_type().await?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if Path::new(name).extension().and_then(|ext| ext.to_str()) != Some("jpg")
            || owned.contains(name)
        {
            continue;
        }
        let metadata = entry.metadata().await?;
        let expired = metadata
            .modified()
            .ok()
            .and_then(|modified| wall_now.duration_since(modified).ok())
            .is_some_and(|age| age >= TERMINAL_TTL);
        if !expired {
            continue;
        }
        match remove_owned_file(&state.paths.captures_tmp_root, &entry.path()) {
            Ok(()) => stats.jpegs_removed += 1,
            Err(error) => {
                warn_delete_failure("orphan", anyhow_error_kind(&error));
                stats.delete_failures += 1;
            }
        }
    }
    Ok(())
}

pub async fn shutdown_captures(state: &AppState) -> anyhow::Result<()> {
    state.captures.stop_and_join().await;

    let captures: Vec<(String, Option<PathBuf>)> = {
        let entries = state.captures.read().await;
        entries
            .iter()
            .map(|(id, capture)| (id.clone(), capture.photo_path.as_deref().map(PathBuf::from)))
            .collect()
    };
    for (id, photo_path) in captures {
        if let Some(path) = photo_path {
            if !is_direct_jpeg_child(&state.paths.captures_tmp_root, &path) {
                anyhow::bail!("capture {id} owns a JPEG outside captures_tmp_root");
            }
            match remove_owned_file(&state.paths.captures_tmp_root, &path) {
                Ok(()) => {}
                Err(error) => {
                    warn_delete_failure(&id, anyhow_error_kind(&error));
                    return Err(error.into());
                }
            }
        }
        state.captures.write().await.remove(&id);
    }

    let mut stats = CleanupStats::default();
    sweep_orphan_jpegs(state, SystemTime::now(), &mut stats).await?;
    Ok(())
}

/// Complete the crash-recovery orphan sweep before any HTTP admission or
/// background producer starts.
pub async fn startup_sweep(state: &AppState, now: CleanupNow) -> anyhow::Result<CleanupStats> {
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root).await?;
    let mut stats = CleanupStats::default();
    sweep_orphan_jpegs(state, now.wall, &mut stats).await?;
    Ok(stats)
}

pub async fn run(state: AppState, shutdown: CancellationToken) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(CLEANUP_CADENCE);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                cleanup_active_states(
                    &state,
                    Instant::now(),
                    &mut CleanupStats::default(),
                    &mut |_| std::future::ready(()),
                ).await?;
                return Ok(());
            }
            _ = interval.tick() => {
                cleanup_active_states(
                    &state,
                    Instant::now(),
                    &mut CleanupStats::default(),
                    &mut |_| std::future::ready(()),
                ).await?;
            }
        }
    }
}

fn is_direct_jpeg_child(root: &Path, path: &Path) -> bool {
    path.parent() == Some(root)
        && path.extension().and_then(|extension| extension.to_str()) == Some("jpg")
        && path.file_stem().is_some()
}

fn warn_delete_failure(capture_id: &str, reason: &str) {
    tracing::warn!(capture_id, reason, "capture JPEG cleanup deferred");
}

fn error_kind_label(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::PermissionDenied => "permission-denied",
        ErrorKind::IsADirectory => "is-directory",
        ErrorKind::NotFound => "not-found",
        _ => "io-error",
    }
}

fn anyhow_error_kind(error: &anyhow::Error) -> &'static str {
    error
        .downcast_ref::<std::io::Error>()
        .map(|error| error_kind_label(error.kind()))
        .unwrap_or("io-error")
}
