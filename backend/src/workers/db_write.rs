use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

use crate::db::write_queue::{run_write_worker, WriteCommand};

pub async fn run(
    db: std::sync::Arc<libsql::Database>,
    rx: UnboundedReceiver<WriteCommand>,
    shutdown: CancellationToken,
) {
    run_write_worker(db, rx, shutdown).await;
}
