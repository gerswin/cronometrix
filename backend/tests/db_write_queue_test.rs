use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::response::IntoResponse;
use cronometrix_api::db::write_queue::{
    run_write_worker, DbWriteError, DbWriteQueue, DbWriteQueueConfig, BACKGROUND_RETRY_DELAYS,
};
use cronometrix_api::errors::AppError;
use http_body_util::BodyExt;

async fn test_db() -> Arc<libsql::Database> {
    let path = format!("/tmp/cronometrix_write_queue_{}.db", uuid::Uuid::new_v4());
    let db = libsql::Builder::new_local(path).build().await.unwrap();
    Arc::new(db)
}

fn config(capacity: usize, enqueue_timeout: Duration) -> DbWriteQueueConfig {
    DbWriteQueueConfig {
        capacity,
        enqueue_timeout,
        background_retry_delays: BACKGROUND_RETRY_DELAYS,
    }
}

async fn wait_for_accepted(queue: &DbWriteQueue, expected: u64) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while queue.stats().accepted < expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("jobs should be admitted");
}

#[tokio::test]
async fn typed_job_returns_a_string() {
    let db = test_db().await;
    let (queue, rx) = DbWriteQueue::channel(config(8, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));

    let value = queue
        .job("typed-string", |_conn| {
            Box::pin(async { Ok("typed result".to_string()) })
        })
        .await
        .unwrap();

    assert_eq!(value, "typed result");
    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn typed_transaction_returns_a_string_and_commits() {
    let db = test_db().await;
    let conn = db.connect().unwrap();
    conn.execute("CREATE TABLE writes (value TEXT NOT NULL)", ())
        .await
        .unwrap();
    let (queue, rx) = DbWriteQueue::channel(config(8, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));

    let value = queue
        .transact("typed-transaction", |tx| {
            Box::pin(async move {
                tx.statement(
                    "INSERT INTO writes (value) VALUES (?1)",
                    vec![libsql::Value::Text("committed".to_string())],
                )
                .await?;
                Ok("transaction result".to_string())
            })
        })
        .await
        .unwrap();

    assert_eq!(value, "transaction result");
    let mut rows = conn.query("SELECT value FROM writes", ()).await.unwrap();
    assert_eq!(
        rows.next()
            .await
            .unwrap()
            .unwrap()
            .get::<String>(0)
            .unwrap(),
        "committed"
    );
    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn after_commit_runs_with_cancelled_reply_before_flush_and_shutdown() {
    let db = test_db().await;
    let conn = db.connect().unwrap();
    conn.execute("CREATE TABLE writes (value TEXT NOT NULL)", ())
        .await
        .unwrap();
    let (queue, rx) = DbWriteQueue::channel(config(8, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let (callback_tx, mut callback_rx) = tokio::sync::mpsc::unbounded_channel();

    let producer_queue = queue.clone();
    let producer = tokio::spawn(async move {
        producer_queue
            .transact("cancelled-reply-post-commit", move |tx| {
                tx.after_commit(move || {
                    callback_tx.send("committed").unwrap();
                });
                Box::pin(async move {
                    started_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    tx.statement("INSERT INTO writes (value) VALUES ('kept')", ())
                        .await?;
                    Ok(())
                })
            })
            .await
    });
    started_rx.await.unwrap();
    producer.abort();
    assert!(producer.await.unwrap_err().is_cancelled());
    release_tx.send(()).unwrap();

    queue.flush().await.unwrap();
    assert_eq!(callback_rx.try_recv().unwrap(), "committed");
    let count: i64 = conn
        .query("SELECT COUNT(*) FROM writes", ())
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(count, 1);

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn after_commit_is_dropped_without_running_on_transaction_rollback() {
    let db = test_db().await;
    let (queue, rx) = DbWriteQueue::channel(config(8, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));
    let callbacks = Arc::new(AtomicUsize::new(0));
    let callback_count = callbacks.clone();

    let error = queue
        .transact::<(), _>("rollback-post-commit", move |tx| {
            tx.after_commit(move || {
                callback_count.fetch_add(1, Ordering::SeqCst);
            });
            Box::pin(async { anyhow::bail!("force rollback") })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Job(_)));
    queue.flush().await.unwrap();
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn after_commit_is_dropped_without_running_when_commit_fails() {
    let db = test_db().await;
    let conn = db.connect().unwrap();
    conn.execute("CREATE TABLE parents (id INTEGER PRIMARY KEY)", ())
        .await
        .unwrap();
    conn.execute(
        "CREATE TABLE children (parent_id INTEGER, \
         FOREIGN KEY(parent_id) REFERENCES parents(id) DEFERRABLE INITIALLY DEFERRED)",
        (),
    )
    .await
    .unwrap();
    let (queue, rx) = DbWriteQueue::channel(config(8, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));
    queue
        .job("enable-foreign-keys", |conn| {
            Box::pin(async move {
                conn.statement("PRAGMA foreign_keys = ON", ()).await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    let callbacks = Arc::new(AtomicUsize::new(0));
    let callback_count = callbacks.clone();

    let error = queue
        .transact::<(), _>("commit-failure-post-commit", move |tx| {
            tx.after_commit(move || {
                callback_count.fetch_add(1, Ordering::SeqCst);
            });
            Box::pin(async move {
                tx.statement("INSERT INTO children (parent_id) VALUES (999)", ())
                    .await?;
                Ok(())
            })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Job(_)));
    queue.flush().await.unwrap();
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn panicking_after_commit_callback_is_isolated_from_the_worker() {
    let db = test_db().await;
    let (queue, rx) = DbWriteQueue::channel(config(8, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));

    let value = queue
        .transact("panicking-post-commit", |tx| {
            tx.after_commit(|| panic!("expected callback panic"));
            Box::pin(async { Ok(42_u8) })
        })
        .await
        .unwrap();
    assert_eq!(value, 42);
    assert_eq!(
        queue
            .job("worker-still-alive", |_conn| Box::pin(async { Ok(7_u8) }))
            .await
            .unwrap(),
        7
    );

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn capacity_one_queue_times_out_the_second_producer_as_busy() {
    let (queue, _rx) = DbWriteQueue::channel(config(1, Duration::from_millis(10)));
    let first_queue = queue.clone();
    let first = tokio::spawn(async move {
        first_queue
            .job("first", |_conn| {
                Box::pin(async { Ok::<_, anyhow::Error>(()) })
            })
            .await
    });
    wait_for_accepted(&queue, 1).await;

    let error = queue
        .job("second", |_conn| {
            Box::pin(async { Ok::<_, anyhow::Error>(()) })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Busy));

    first.abort();
}

#[tokio::test]
async fn enqueue_timeout_includes_waiting_for_the_admission_lock() {
    let enqueue_timeout = Duration::from_millis(100);
    let (queue, _rx) = DbWriteQueue::channel(config(1, enqueue_timeout));
    let first_queue = queue.clone();
    let first = tokio::spawn(async move {
        first_queue
            .job("fills-capacity", |_conn| {
                Box::pin(async { Ok::<_, anyhow::Error>(()) })
            })
            .await
    });
    wait_for_accepted(&queue, 1).await;

    let second_queue = queue.clone();
    let second = tokio::spawn(async move {
        second_queue
            .job("holds-admission-lock", |_conn| {
                Box::pin(async { Ok::<_, anyhow::Error>(()) })
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;

    let started = tokio::time::Instant::now();
    let error = queue
        .job("waits-for-admission-lock", |_conn| {
            Box::pin(async { Ok::<_, anyhow::Error>(()) })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Busy));
    assert!(
        started.elapsed() < Duration::from_millis(140),
        "the lock wait and reserve wait must share one admission timeout"
    );

    assert!(matches!(second.await.unwrap(), Err(DbWriteError::Busy)));
    first.abort();
}

#[tokio::test]
async fn busy_serializes_as_stable_503_application_error() {
    let response = AppError::from(DbWriteError::Busy).into_response();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "DB_WRITE_QUEUE_BUSY");
    assert_eq!(json["error"]["status"], 503);
}

#[tokio::test]
async fn unavailable_errors_serialize_as_stable_503_application_errors() {
    for error in [DbWriteError::Closed, DbWriteError::WorkerStopped] {
        let response = AppError::from(error).into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "DB_WRITE_QUEUE_UNAVAILABLE");
    }
}

#[tokio::test]
async fn domain_errors_survive_typed_job_erasure() {
    let db = test_db().await;
    let (queue, rx) = DbWriteQueue::channel(config(8, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));

    let queue_error = queue
        .job::<(), _>("domain-conflict", |_conn| {
            Box::pin(async {
                Err(anyhow::Error::new(AppError::Conflict {
                    code: "VERSION_CONFLICT",
                    message: "stale version".to_string(),
                }))
            })
        })
        .await
        .unwrap_err();
    let response = AppError::from(queue_error).into_response();
    assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "VERSION_CONFLICT");

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn failed_job_does_not_kill_the_worker() {
    let db = test_db().await;
    let (queue, rx) = DbWriteQueue::channel(config(8, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));

    let error = queue
        .job::<(), _>("fails", |_conn| {
            Box::pin(async { anyhow::bail!("expected") })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Job(_)));

    let value = queue
        .job("still-alive", |_conn| Box::pin(async { Ok(42_u64) }))
        .await
        .unwrap();
    assert_eq!(value, 42);

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn flush_observes_every_earlier_accepted_command() {
    let db = test_db().await;
    let conn = db.connect().unwrap();
    conn.execute("CREATE TABLE writes (value INTEGER NOT NULL)", ())
        .await
        .unwrap();
    let (queue, rx) = DbWriteQueue::channel(config(32, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db.clone(), rx));

    let mut producers = Vec::new();
    for value in 0..20_i64 {
        let queue = queue.clone();
        producers.push(tokio::spawn(async move {
            queue
                .statement(
                    "insert-write",
                    "INSERT INTO writes (value) VALUES (?1)",
                    vec![libsql::Value::Integer(value)],
                )
                .await
        }));
    }
    wait_for_accepted(&queue, 20).await;
    queue.flush().await.unwrap();

    let mut rows = conn.query("SELECT COUNT(*) FROM writes", ()).await.unwrap();
    let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(count, 20);
    for producer in producers {
        producer.await.unwrap().unwrap();
    }

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn concurrent_close_persists_exactly_all_accepted_jobs() {
    let db = test_db().await;
    let conn = db.connect().unwrap();
    conn.execute("CREATE TABLE writes (value INTEGER NOT NULL)", ())
        .await
        .unwrap();
    let (queue, rx) = DbWriteQueue::channel(config(4, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db.clone(), rx));

    let mut producers = Vec::new();
    for value in 0..100_i64 {
        let queue = queue.clone();
        producers.push(tokio::spawn(async move {
            queue
                .statement(
                    "concurrent-insert",
                    "INSERT INTO writes (value) VALUES (?1)",
                    vec![libsql::Value::Integer(value)],
                )
                .await
        }));
    }
    tokio::task::yield_now().await;
    queue.close_and_flush().await.unwrap();

    let mut accepted = 0_i64;
    for producer in producers {
        match producer.await.unwrap() {
            Ok(_) => accepted += 1,
            Err(DbWriteError::Closed) => {}
            Err(other) => panic!("unexpected producer result: {other:?}"),
        }
    }
    worker.await.unwrap().unwrap();

    let mut rows = conn.query("SELECT COUNT(*) FROM writes", ()).await.unwrap();
    let persisted: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(persisted, accepted);
}

#[tokio::test]
async fn enqueue_after_close_returns_closed() {
    let db = test_db().await;
    let (queue, rx) = DbWriteQueue::channel(config(2, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));
    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();

    let error = queue
        .job("too-late", |_conn| {
            Box::pin(async { Ok::<_, anyhow::Error>(()) })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Closed));

    assert!(matches!(queue.flush().await, Err(DbWriteError::Closed)));
    assert!(matches!(
        queue.close_and_flush().await,
        Err(DbWriteError::Closed)
    ));
}

#[tokio::test]
async fn workers_db_write_wrapper_drains_and_stops() {
    let db = test_db().await;
    let (queue, rx) = DbWriteQueue::channel(config(2, Duration::from_secs(1)));
    let worker = tokio::spawn(cronometrix_api::workers::db_write::run(db, rx));

    assert_eq!(
        queue
            .job("wrapper-round-trip", |_conn| Box::pin(async { Ok(7_u8) }))
            .await
            .unwrap(),
        7
    );
    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_waits_for_a_transient_external_lock_instead_of_failing_immediately() {
    let db = test_db().await;
    let blocker = db.connect().unwrap();
    blocker
        .execute("CREATE TABLE transient_lock (value INTEGER NOT NULL)", ())
        .await
        .unwrap();
    let (queue, rx) = DbWriteQueue::channel(config(2, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));
    queue.flush().await.unwrap();

    blocker.execute_batch("BEGIN IMMEDIATE").await.unwrap();
    let write_queue = queue.clone();
    let write = tokio::spawn(async move {
        write_queue
            .statement(
                "transient-lock-regression",
                "INSERT INTO transient_lock (value) VALUES (1)",
                Vec::new(),
            )
            .await
    });
    wait_for_accepted(&queue, 1).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !write.is_finished(),
        "writer must wait for a short external lock"
    );

    blocker.execute_batch("COMMIT").await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), write)
            .await
            .expect("writer resumes after lock release")
            .unwrap()
            .unwrap(),
        1
    );
    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn stats_count_depth_and_every_terminal_outcome() {
    let db = test_db().await;
    let (queue, rx) = DbWriteQueue::channel(config(1, Duration::from_millis(10)));
    let first_queue = queue.clone();
    let first = tokio::spawn(async move {
        first_queue
            .job::<(), _>("fails", |_conn| {
                Box::pin(async { anyhow::bail!("expected") })
            })
            .await
    });
    wait_for_accepted(&queue, 1).await;
    assert_eq!(queue.stats().depth, 1);

    let error = queue
        .job("busy", |_conn| {
            Box::pin(async { Ok::<_, anyhow::Error>(()) })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Busy));

    let worker = tokio::spawn(run_write_worker(db, rx));
    assert!(matches!(first.await.unwrap(), Err(DbWriteError::Job(_))));
    queue
        .job("succeeds", |_conn| {
            Box::pin(async { Ok::<_, anyhow::Error>(()) })
        })
        .await
        .unwrap();
    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();

    assert!(matches!(
        queue
            .job("closed", |_conn| Box::pin(async {
                Ok::<_, anyhow::Error>(())
            }))
            .await,
        Err(DbWriteError::Closed)
    ));
    let stats = queue.stats();
    assert_eq!(stats.depth, 0);
    assert_eq!(stats.accepted, 2);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.busy_rejections, 1);
    assert_eq!(stats.closed_rejections, 1);
}

#[tokio::test]
async fn background_admission_retries_busy_three_times_and_no_other_error() {
    let (queue, _rx) = DbWriteQueue::channel(config(1, Duration::from_millis(5)));
    let first_queue = queue.clone();
    let first = tokio::spawn(async move {
        first_queue
            .job("fills-capacity", |_conn| {
                Box::pin(async { Ok::<_, anyhow::Error>(()) })
            })
            .await
    });
    wait_for_accepted(&queue, 1).await;

    let started = tokio::time::Instant::now();
    let error = queue
        .background_job("background", |_conn| {
            Box::pin(async { Ok::<_, anyhow::Error>(()) })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Busy));
    assert_eq!(queue.stats().busy_rejections, 4);
    assert!(started.elapsed() >= BACKGROUND_RETRY_DELAYS.into_iter().sum::<Duration>());
    first.abort();

    let db = test_db().await;
    let (closed_queue, rx) = DbWriteQueue::channel(config(1, Duration::from_millis(5)));
    let worker = tokio::spawn(run_write_worker(db, rx));
    closed_queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
    let error = closed_queue
        .background_job("closed", |_conn| {
            Box::pin(async { Ok::<_, anyhow::Error>(()) })
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DbWriteError::Closed));
    assert_eq!(closed_queue.stats().closed_rejections, 1);
    assert_eq!(closed_queue.stats().busy_rejections, 0);
}

#[tokio::test]
async fn queued_connection_query_returns_rows_from_the_writer_connection() {
    let db = test_db().await;
    db.connect()
        .unwrap()
        .execute("CREATE TABLE query_values (value TEXT NOT NULL)", ())
        .await
        .unwrap();
    let (queue, rx) = DbWriteQueue::channel(config(2, Duration::from_secs(1)));
    let worker = tokio::spawn(run_write_worker(db, rx));

    let value = queue
        .job("query-writer", |conn| {
            Box::pin(async move {
                conn.statement("INSERT INTO query_values VALUES ('visible')", ())
                    .await?;
                let mut rows = conn.query("SELECT value FROM query_values", ()).await?;
                Ok(rows.next().await?.unwrap().get::<String>(0)?)
            })
        })
        .await
        .unwrap();
    assert_eq!(value, "visible");

    queue.close_and_flush().await.unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn flush_times_out_when_queue_capacity_is_exhausted() {
    let (queue, _rx) = DbWriteQueue::channel(config(1, Duration::from_millis(20)));
    let producer_queue = queue.clone();
    let producer = tokio::spawn(async move {
        producer_queue
            .job("fills-control-capacity", |_conn| {
                Box::pin(async { Ok::<_, anyhow::Error>(()) })
            })
            .await
    });
    wait_for_accepted(&queue, 1).await;

    assert!(matches!(queue.flush().await, Err(DbWriteError::Busy)));
    assert_eq!(queue.stats().busy_rejections, 1);
    producer.abort();
}

#[tokio::test]
async fn flush_times_out_while_an_admission_waiter_holds_the_lock() {
    let timeout = Duration::from_millis(80);
    let (queue, _rx) = DbWriteQueue::channel(config(1, timeout));
    let first_queue = queue.clone();
    let first = tokio::spawn(async move {
        first_queue
            .job("fills-capacity-before-flush", |_conn| {
                Box::pin(async { Ok::<_, anyhow::Error>(()) })
            })
            .await
    });
    wait_for_accepted(&queue, 1).await;

    let waiting_queue = queue.clone();
    let waiting = tokio::spawn(async move {
        waiting_queue
            .job("holds-lock-before-flush", |_conn| {
                Box::pin(async { Ok::<_, anyhow::Error>(()) })
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert!(matches!(queue.flush().await, Err(DbWriteError::Busy)));
    assert!(matches!(waiting.await.unwrap(), Err(DbWriteError::Busy)));
    first.abort();
}

#[tokio::test]
async fn dropped_worker_receiver_rejects_jobs_and_control_commands() {
    let (job_queue, job_rx) = DbWriteQueue::channel(config(1, Duration::from_secs(1)));
    drop(job_rx);
    let job_error = job_queue
        .job("stopped-job", |_conn| {
            Box::pin(async { Ok::<_, anyhow::Error>(()) })
        })
        .await
        .unwrap_err();
    assert!(matches!(job_error, DbWriteError::WorkerStopped));

    let (flush_queue, flush_rx) = DbWriteQueue::channel(config(1, Duration::from_secs(1)));
    drop(flush_rx);
    assert!(matches!(
        flush_queue.flush().await,
        Err(DbWriteError::WorkerStopped)
    ));
}

#[test]
#[should_panic(expected = "write queue capacity must be positive")]
fn zero_capacity_is_rejected() {
    let _ = DbWriteQueue::channel(config(0, Duration::from_secs(1)));
}
