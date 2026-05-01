use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// Single-writer queue for SQLite/libSQL mutations.
///
/// The queue serializes all write statements onto one task so handlers/workers
/// no longer compete directly for database write locks.
#[derive(Clone)]
pub struct DbWriteQueue {
    tx: mpsc::UnboundedSender<WriteCommand>,
}

pub enum WriteCommand {
    Execute {
        sql: String,
        params: Vec<libsql::Value>,
        reply: oneshot::Sender<anyhow::Result<u64>>,
    },
    ExecuteBatch {
        sql: String,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    Run {
        job: Box<DbJob>,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
}

pub type DbJob = dyn for<'a> FnOnce(
        &'a libsql::Connection,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>
    + Send;

impl DbWriteQueue {
    pub fn new(tx: mpsc::UnboundedSender<WriteCommand>) -> Self {
        Self { tx }
    }

    pub async fn execute(
        &self,
        sql: impl Into<String>,
        params: Vec<libsql::Value>,
    ) -> anyhow::Result<u64> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(WriteCommand::Execute {
                sql: sql.into(),
                params,
                reply: reply_tx,
            })
            .map_err(|_| anyhow::anyhow!("write queue closed"))?;
        reply_rx.await.map_err(|_| anyhow::anyhow!("write queue dropped"))?
    }

    pub async fn execute_batch(&self, sql: impl Into<String>) -> anyhow::Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(WriteCommand::ExecuteBatch {
                sql: sql.into(),
                reply: reply_tx,
            })
            .map_err(|_| anyhow::anyhow!("write queue closed"))?;
        reply_rx.await.map_err(|_| anyhow::anyhow!("write queue dropped"))?
    }

    pub async fn run<F>(&self, job: F) -> anyhow::Result<()>
    where
        F: for<'a> FnOnce(
                &'a libsql::Connection,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>
            + Send
            + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(WriteCommand::Run {
                job: Box::new(job),
                reply: reply_tx,
            })
            .map_err(|_| anyhow::anyhow!("write queue closed"))?;
        reply_rx.await.map_err(|_| anyhow::anyhow!("write queue dropped"))?
    }
}

pub async fn run_write_worker(
    db: Arc<libsql::Database>,
    mut rx: mpsc::UnboundedReceiver<WriteCommand>,
    shutdown: CancellationToken,
) {
    tracing::info!("DbWriteQueue worker started");
    let conn = match db.connect() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(err = %e, "DbWriteQueue: failed to open writer connection");
            return;
        }
    };

    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => {
                tracing::info!("DbWriteQueue worker shutting down");
                return;
            }
            msg = rx.recv() => {
                let Some(msg) = msg else {
                    tracing::info!("DbWriteQueue channel closed");
                    return;
                };

                match msg {
                    WriteCommand::Execute { sql, params, reply } => {
                        let result = conn
                            .execute(&sql, libsql::params_from_iter(params))
                            .await
                            .map_err(|e| anyhow::anyhow!(e));
                        let _ = reply.send(result);
                    }
                    WriteCommand::ExecuteBatch { sql, reply } => {
                        let result = conn.execute_batch(&sql).await.map_err(|e| anyhow::anyhow!(e));
                        let _ = reply.send(result.map(|_| ()));
                    }
                    WriteCommand::Run { job, reply } => {
                        let result = job(&conn).await;
                        let _ = reply.send(result);
                    }
                }
            }
        }
    }
}
