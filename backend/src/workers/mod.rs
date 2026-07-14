pub mod backfill;
pub mod capture_cleanup;
/// Drained bounded database writer; shutdown is commanded by `DbWriteQueue`.
pub mod db_write;
pub mod purge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSource {
    Interrupt,
    Terminate,
}

/// Testable first-signal selector. Production wires Ctrl-C and SIGTERM into
/// these futures; tests use ready/pending futures without sending process-wide
/// signals to the test runner.
pub async fn first_shutdown_signal<C, T>(ctrl_c: C, terminate: T) -> ShutdownSource
where
    C: std::future::Future<Output = ()>,
    T: std::future::Future<Output = ()>,
{
    tokio::select! {
        _ = ctrl_c => ShutdownSource::Interrupt,
        _ = terminate => ShutdownSource::Terminate,
    }
}

#[cfg(unix)]
pub async fn shutdown_signal() -> anyhow::Result<ShutdownSource> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result?;
            Ok(ShutdownSource::Interrupt)
        }
        _ = terminate.recv() => Ok(ShutdownSource::Terminate),
    }
}

#[cfg(not(unix))]
pub async fn shutdown_signal() -> anyhow::Result<ShutdownSource> {
    tokio::signal::ctrl_c().await?;
    Ok(ShutdownSource::Interrupt)
}
