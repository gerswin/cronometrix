use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot, Mutex};

pub const DEFAULT_WRITE_QUEUE_CAPACITY: usize = 1024;
pub const DEFAULT_ENQUEUE_TIMEOUT: Duration = Duration::from_secs(5);
pub const BACKGROUND_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(250),
    Duration::from_millis(500),
];

#[derive(Clone, Copy, Debug)]
pub struct DbWriteQueueConfig {
    pub capacity: usize,
    pub enqueue_timeout: Duration,
    pub background_retry_delays: [Duration; 3],
}

impl Default for DbWriteQueueConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_WRITE_QUEUE_CAPACITY,
            enqueue_timeout: DEFAULT_ENQUEUE_TIMEOUT,
            background_retry_delays: BACKGROUND_RETRY_DELAYS,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DbWriteError {
    #[error("database write queue is busy")]
    Busy,
    #[error("database write queue is closed")]
    Closed,
    #[error("database write worker stopped")]
    WorkerStopped,
    #[error("database write job failed: {0:#}")]
    Job(anyhow::Error),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DbWriteQueueStats {
    pub depth: usize,
    pub accepted: u64,
    pub completed: u64,
    pub failed: u64,
    pub busy_rejections: u64,
    pub closed_rejections: u64,
}

#[derive(Default)]
struct Stats {
    depth: AtomicUsize,
    accepted: AtomicU64,
    completed: AtomicU64,
    failed: AtomicU64,
    busy_rejections: AtomicU64,
    closed_rejections: AtomicU64,
}

impl Stats {
    fn snapshot(&self) -> DbWriteQueueStats {
        DbWriteQueueStats {
            depth: self.depth.load(Ordering::Relaxed),
            accepted: self.accepted.load(Ordering::Relaxed),
            completed: self.completed.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            busy_rejections: self.busy_rejections.load(Ordering::Relaxed),
            closed_rejections: self.closed_rejections.load(Ordering::Relaxed),
        }
    }
}

struct QueueInner {
    tx: mpsc::Sender<WriteCommand>,
    config: DbWriteQueueConfig,
    admission: Mutex<()>,
    closed: AtomicBool,
    stats: Arc<Stats>,
}

/// Cloneable, bounded single-writer admission handle.
#[derive(Clone)]
pub struct DbWriteQueue {
    inner: Arc<QueueInner>,
}

pub struct DbWriteQueueReceiver {
    rx: mpsc::Receiver<WriteCommand>,
    stats: Arc<Stats>,
}

type ErasedValue = Box<dyn Any + Send>;
type ErasedFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<ErasedValue>> + Send + 'a>>;
pub type QueuedFuture<'a, T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + 'a>>;

trait ErasedJob: Send {
    fn run<'a>(self: Box<Self>, conn: &'a QueuedConnection<'a>) -> ErasedFuture<'a>;
}

struct TypedJob<F, T> {
    job: F,
    output: PhantomData<T>,
}

impl<F, T> ErasedJob for TypedJob<F, T>
where
    T: Send + 'static,
    F: for<'a> FnOnce(&'a QueuedConnection<'a>) -> QueuedFuture<'a, T> + Send + 'static,
{
    fn run<'a>(self: Box<Self>, conn: &'a QueuedConnection<'a>) -> ErasedFuture<'a> {
        let TypedJob { job, .. } = *self;
        Box::pin(async move {
            let output = job(conn).await?;
            Ok(Box::new(output) as ErasedValue)
        })
    }
}

trait ErasedTransaction: Send {
    fn run<'a>(self: Box<Self>, tx: &'a QueuedTransaction<'a>) -> ErasedFuture<'a>;
}

struct TypedTransaction<F, T> {
    job: F,
    output: PhantomData<T>,
}

impl<F, T> ErasedTransaction for TypedTransaction<F, T>
where
    T: Send + 'static,
    F: for<'a> FnOnce(&'a QueuedTransaction<'a>) -> QueuedFuture<'a, T> + Send + 'static,
{
    fn run<'a>(self: Box<Self>, tx: &'a QueuedTransaction<'a>) -> ErasedFuture<'a> {
        let TypedTransaction { job, .. } = *self;
        Box::pin(async move {
            let output = job(tx).await?;
            Ok(Box::new(output) as ErasedValue)
        })
    }
}

enum WriteCommand {
    Job {
        operation: String,
        job: Box<dyn ErasedJob>,
        reply: oneshot::Sender<anyhow::Result<ErasedValue>>,
    },
    Transaction {
        operation: String,
        job: Box<dyn ErasedTransaction>,
        reply: oneshot::Sender<anyhow::Result<ErasedValue>>,
    },
    Flush(oneshot::Sender<()>),
    Shutdown(oneshot::Sender<()>),
}

/// Read/write helpers available inside a queued non-transactional job.
/// The raw writer connection intentionally remains private.
pub struct QueuedConnection<'a> {
    connection: &'a libsql::Connection,
}

impl QueuedConnection<'_> {
    pub async fn statement(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> anyhow::Result<u64> {
        self.connection
            .execute(sql, params)
            .await
            .map_err(Into::into)
    }

    pub async fn query(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> anyhow::Result<libsql::Rows> {
        self.connection.query(sql, params).await.map_err(Into::into)
    }
}

/// Read/write helpers available inside a queued transaction.
/// Commit and rollback remain owned by the queue worker.
pub struct QueuedTransaction<'a> {
    transaction: &'a libsql::Transaction,
    after_commit: std::sync::Mutex<Vec<AfterCommitCallback>>,
}

type AfterCommitCallback = Box<dyn FnOnce() + Send + 'static>;

impl QueuedTransaction<'_> {
    /// Register synchronous work that the single-writer worker runs only after
    /// this transaction commits, before replying or processing the next command.
    pub fn after_commit(&self, callback: impl FnOnce() + Send + 'static) {
        self.after_commit
            .lock()
            .expect("after_commit callback registry poisoned")
            .push(Box::new(callback));
    }

    fn take_after_commit(&self) -> Vec<AfterCommitCallback> {
        std::mem::take(
            &mut *self
                .after_commit
                .lock()
                .expect("after_commit callback registry poisoned"),
        )
    }

    pub async fn statement(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> anyhow::Result<u64> {
        self.transaction
            .execute(sql, params)
            .await
            .map_err(Into::into)
    }

    pub async fn query(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> anyhow::Result<libsql::Rows> {
        self.transaction
            .query(sql, params)
            .await
            .map_err(Into::into)
    }
}

impl DbWriteQueue {
    pub fn channel(config: DbWriteQueueConfig) -> (Self, DbWriteQueueReceiver) {
        assert!(config.capacity > 0, "write queue capacity must be positive");
        let (tx, rx) = mpsc::channel(config.capacity);
        let stats = Arc::new(Stats::default());
        let queue = Self {
            inner: Arc::new(QueueInner {
                tx,
                config,
                admission: Mutex::new(()),
                closed: AtomicBool::new(false),
                stats: stats.clone(),
            }),
        };
        (queue, DbWriteQueueReceiver { rx, stats })
    }

    pub fn stats(&self) -> DbWriteQueueStats {
        self.inner.stats.snapshot()
    }

    async fn admit_once<F>(
        &self,
        operation: &str,
        command: &mut Option<F>,
    ) -> Result<(), DbWriteError>
    where
        F: FnOnce() -> WriteCommand,
    {
        let started = Instant::now();
        let deadline = tokio::time::Instant::now() + self.inner.config.enqueue_timeout;
        let _admission = match tokio::time::timeout_at(deadline, self.inner.admission.lock()).await
        {
            Ok(admission) => admission,
            Err(_) => {
                self.inner
                    .stats
                    .busy_rejections
                    .fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    operation,
                    wait_ms = started.elapsed().as_millis() as u64,
                    "database write queue admission timed out"
                );
                return Err(DbWriteError::Busy);
            }
        };
        if self.inner.closed.load(Ordering::Acquire) {
            self.inner
                .stats
                .closed_rejections
                .fetch_add(1, Ordering::Relaxed);
            tracing::warn!(operation, "database write rejected after queue close");
            return Err(DbWriteError::Closed);
        }

        let permit =
            match tokio::time::timeout_at(deadline, self.inner.tx.clone().reserve_owned()).await {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => return Err(DbWriteError::WorkerStopped),
                Err(_) => {
                    self.inner
                        .stats
                        .busy_rejections
                        .fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        operation,
                        wait_ms = started.elapsed().as_millis() as u64,
                        "database write queue admission timed out"
                    );
                    return Err(DbWriteError::Busy);
                }
            };

        self.inner.stats.accepted.fetch_add(1, Ordering::Relaxed);
        self.inner.stats.depth.fetch_add(1, Ordering::Relaxed);
        permit.send(command.take().expect("write command admitted once")());
        tracing::debug!(
            operation,
            wait_ms = started.elapsed().as_millis() as u64,
            depth = self.inner.stats.depth.load(Ordering::Relaxed),
            "database write accepted"
        );
        Ok(())
    }

    async fn admit<F>(
        &self,
        operation: &str,
        background: bool,
        command: F,
    ) -> Result<(), DbWriteError>
    where
        F: FnOnce() -> WriteCommand,
    {
        let mut command = Some(command);
        match self.admit_once(operation, &mut command).await {
            Err(DbWriteError::Busy) if background => {}
            result => return result,
        }

        for delay in self.inner.config.background_retry_delays {
            tokio::time::sleep(delay).await;
            match self.admit_once(operation, &mut command).await {
                Err(DbWriteError::Busy) => continue,
                result => return result,
            }
        }
        Err(DbWriteError::Busy)
    }

    async fn submit_job<T, F>(
        &self,
        operation: impl Into<String>,
        background: bool,
        job: F,
    ) -> Result<T, DbWriteError>
    where
        T: Send + 'static,
        F: for<'a> FnOnce(&'a QueuedConnection<'a>) -> QueuedFuture<'a, T> + Send + 'static,
    {
        let operation = operation.into();
        let (reply_tx, reply_rx) = oneshot::channel();
        self.admit(&operation, background, || WriteCommand::Job {
            operation: operation.clone(),
            job: Box::new(TypedJob {
                job,
                output: PhantomData,
            }),
            reply: reply_tx,
        })
        .await?;

        let output = reply_rx.await.map_err(|_| DbWriteError::WorkerStopped)?;
        output
            .map_err(DbWriteError::Job)?
            .downcast::<T>()
            .map(|value| *value)
            .map_err(|_| DbWriteError::WorkerStopped)
    }

    pub async fn job<T, F>(&self, operation: impl Into<String>, job: F) -> Result<T, DbWriteError>
    where
        T: Send + 'static,
        F: for<'a> FnOnce(&'a QueuedConnection<'a>) -> QueuedFuture<'a, T> + Send + 'static,
    {
        self.submit_job(operation, false, job).await
    }

    pub async fn background_job<T, F>(
        &self,
        operation: impl Into<String>,
        job: F,
    ) -> Result<T, DbWriteError>
    where
        T: Send + 'static,
        F: for<'a> FnOnce(&'a QueuedConnection<'a>) -> QueuedFuture<'a, T> + Send + 'static,
    {
        self.submit_job(operation, true, job).await
    }

    async fn submit_transaction<T, F>(
        &self,
        operation: impl Into<String>,
        background: bool,
        job: F,
    ) -> Result<T, DbWriteError>
    where
        T: Send + 'static,
        F: for<'a> FnOnce(&'a QueuedTransaction<'a>) -> QueuedFuture<'a, T> + Send + 'static,
    {
        let operation = operation.into();
        let (reply_tx, reply_rx) = oneshot::channel();
        self.admit(&operation, background, || WriteCommand::Transaction {
            operation: operation.clone(),
            job: Box::new(TypedTransaction {
                job,
                output: PhantomData,
            }),
            reply: reply_tx,
        })
        .await?;

        let output = reply_rx.await.map_err(|_| DbWriteError::WorkerStopped)?;
        output
            .map_err(DbWriteError::Job)?
            .downcast::<T>()
            .map(|value| *value)
            .map_err(|_| DbWriteError::WorkerStopped)
    }

    pub async fn transact<T, F>(
        &self,
        operation: impl Into<String>,
        job: F,
    ) -> Result<T, DbWriteError>
    where
        T: Send + 'static,
        F: for<'a> FnOnce(&'a QueuedTransaction<'a>) -> QueuedFuture<'a, T> + Send + 'static,
    {
        self.submit_transaction(operation, false, job).await
    }

    pub async fn background_transact<T, F>(
        &self,
        operation: impl Into<String>,
        job: F,
    ) -> Result<T, DbWriteError>
    where
        T: Send + 'static,
        F: for<'a> FnOnce(&'a QueuedTransaction<'a>) -> QueuedFuture<'a, T> + Send + 'static,
    {
        self.submit_transaction(operation, true, job).await
    }

    async fn submit_statement(
        &self,
        operation: impl Into<String>,
        background: bool,
        sql: impl Into<String>,
        params: Vec<libsql::Value>,
    ) -> Result<u64, DbWriteError> {
        let sql = sql.into();
        self.submit_job(operation, background, move |conn| {
            Box::pin(async move { conn.statement(&sql, params).await })
        })
        .await
    }

    pub async fn statement(
        &self,
        operation: impl Into<String>,
        sql: impl Into<String>,
        params: Vec<libsql::Value>,
    ) -> Result<u64, DbWriteError> {
        self.submit_statement(operation, false, sql, params).await
    }

    pub async fn background_statement(
        &self,
        operation: impl Into<String>,
        sql: impl Into<String>,
        params: Vec<libsql::Value>,
    ) -> Result<u64, DbWriteError> {
        self.submit_statement(operation, true, sql, params).await
    }

    pub async fn flush(&self) -> Result<(), DbWriteError> {
        let operation = "flush";
        let (reply_tx, reply_rx) = oneshot::channel();
        let mut command = Some(|| WriteCommand::Flush(reply_tx));
        self.admit_control(operation, &mut command, false).await?;
        reply_rx.await.map_err(|_| DbWriteError::WorkerStopped)
    }

    async fn admit_control<F>(
        &self,
        operation: &str,
        command: &mut Option<F>,
        closing: bool,
    ) -> Result<(), DbWriteError>
    where
        F: FnOnce() -> WriteCommand,
    {
        let deadline = tokio::time::Instant::now() + self.inner.config.enqueue_timeout;
        let _admission = if closing {
            self.inner.admission.lock().await
        } else {
            match tokio::time::timeout_at(deadline, self.inner.admission.lock()).await {
                Ok(admission) => admission,
                Err(_) => {
                    self.inner
                        .stats
                        .busy_rejections
                        .fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        operation,
                        "database write queue control admission timed out"
                    );
                    return Err(DbWriteError::Busy);
                }
            }
        };
        if !closing && self.inner.closed.load(Ordering::Acquire) {
            self.inner
                .stats
                .closed_rejections
                .fetch_add(1, Ordering::Relaxed);
            return Err(DbWriteError::Closed);
        }
        if closing && self.inner.closed.swap(true, Ordering::AcqRel) {
            return Err(DbWriteError::Closed);
        }

        let reserve = self.inner.tx.clone().reserve_owned();
        let permit = if closing {
            reserve.await.map_err(|_| DbWriteError::WorkerStopped)?
        } else {
            match tokio::time::timeout_at(deadline, reserve).await {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => return Err(DbWriteError::WorkerStopped),
                Err(_) => {
                    self.inner
                        .stats
                        .busy_rejections
                        .fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        operation,
                        "database write queue control admission timed out"
                    );
                    return Err(DbWriteError::Busy);
                }
            }
        };
        permit.send(command.take().expect("control command admitted once")());
        Ok(())
    }

    pub async fn close_and_flush(&self) -> Result<(), DbWriteError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let mut command = Some(|| WriteCommand::Shutdown(reply_tx));
        self.admit_control("shutdown", &mut command, true).await?;
        reply_rx.await.map_err(|_| DbWriteError::WorkerStopped)
    }
}

fn record_result(
    stats: &Stats,
    operation: &str,
    started: Instant,
    result: &anyhow::Result<ErasedValue>,
) {
    match result {
        Ok(_) => {
            stats.completed.fetch_add(1, Ordering::Relaxed);
            tracing::debug!(
                operation,
                duration_ms = started.elapsed().as_millis() as u64,
                "database write completed"
            );
        }
        Err(error) => {
            stats.failed.fetch_add(1, Ordering::Relaxed);
            tracing::error!(
                operation,
                duration_ms = started.elapsed().as_millis() as u64,
                error = %error,
                "database write failed"
            );
        }
    }
}

pub async fn run_write_worker(
    db: Arc<libsql::Database>,
    mut receiver: DbWriteQueueReceiver,
) -> Result<(), DbWriteError> {
    tracing::info!("DbWriteQueue worker started");
    let conn = db.connect().map_err(|error| {
        tracing::error!(err = %error, "DbWriteQueue: failed to open writer connection");
        DbWriteError::WorkerStopped
    })?;
    // `busy_timeout` and `foreign_keys` are connection-local SQLite settings.
    // Configuring them only on the migration connection does not affect this
    // long-lived writer connection. A short external lock (for example an
    // embedded-replica checkpoint) must wait instead of surfacing immediately
    // as `database is locked`.
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; \
         PRAGMA synchronous = NORMAL; \
         PRAGMA busy_timeout = 5000;",
    )
    .await
    .map_err(|error| {
        tracing::error!(err = %error, "DbWriteQueue: failed to configure writer connection");
        DbWriteError::WorkerStopped
    })?;

    while let Some(command) = receiver.rx.recv().await {
        match command {
            WriteCommand::Job {
                operation,
                job,
                reply,
            } => {
                receiver.stats.depth.fetch_sub(1, Ordering::Relaxed);
                let started = Instant::now();
                let queued = QueuedConnection { connection: &conn };
                let result = job.run(&queued).await;
                record_result(&receiver.stats, &operation, started, &result);
                let _ = reply.send(result);
            }
            WriteCommand::Transaction {
                operation,
                job,
                reply,
            } => {
                receiver.stats.depth.fetch_sub(1, Ordering::Relaxed);
                let started = Instant::now();
                let result = match conn.transaction().await {
                    Ok(transaction) => {
                        let queued = QueuedTransaction {
                            transaction: &transaction,
                            after_commit: std::sync::Mutex::new(Vec::new()),
                        };
                        match job.run(&queued).await {
                            Ok(output) => {
                                let after_commit = queued.take_after_commit();
                                match transaction.commit().await {
                                    Ok(()) => {
                                        for callback in after_commit {
                                            if std::panic::catch_unwind(
                                                std::panic::AssertUnwindSafe(callback),
                                            )
                                            .is_err()
                                            {
                                                tracing::error!(
                                                    operation,
                                                    "database after_commit callback panicked"
                                                );
                                            }
                                        }
                                        Ok(output)
                                    }
                                    Err(error) => Err(error.into()),
                                }
                            }
                            Err(error) => {
                                if let Err(rollback_error) = transaction.rollback().await {
                                    tracing::error!(
                                        operation,
                                        error = %rollback_error,
                                        "database transaction rollback failed"
                                    );
                                }
                                Err(error)
                            }
                        }
                    }
                    Err(error) => Err(error.into()),
                };
                record_result(&receiver.stats, &operation, started, &result);
                let _ = reply.send(result);
            }
            WriteCommand::Flush(reply) => {
                let _ = reply.send(());
            }
            WriteCommand::Shutdown(reply) => {
                let _ = reply.send(());
                tracing::info!("DbWriteQueue worker drained and stopped");
                return Ok(());
            }
        }
    }

    tracing::info!("DbWriteQueue channel closed");
    Ok(())
}
