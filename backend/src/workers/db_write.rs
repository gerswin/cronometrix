use crate::db::write_queue::{run_write_worker, DbWriteError, DbWriteQueueReceiver};

pub async fn run(
    db: std::sync::Arc<libsql::Database>,
    rx: DbWriteQueueReceiver,
) -> Result<(), DbWriteError> {
    run_write_worker(db, rx).await
}
