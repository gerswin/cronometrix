use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use axum::{body::Body, http::Request, routing::get, Router};
use tower::ServiceExt;
use tracing_subscriber::fmt::format::FmtSpan;

const MARKER: &str = "SSE_TOKEN_MUST_NOT_APPEAR_IN_LOGS";

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

struct GuardedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for GuardedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedWriter {
    type Writer = GuardedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        GuardedWriter(self.0.clone())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn http_trace_logs_method_and_path_without_query_token() {
    let writer = SharedWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_span_events(FmtSpan::NEW)
        .with_ansi(false)
        .without_time()
        .with_writer(writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let app = Router::new()
        .route("/api/v1/events/stream", get(|| async { "ok" }))
        .layer(cronometrix_api::http_trace::layer());
    let request = Request::builder()
        .uri(format!("/api/v1/events/stream?token={MARKER}"))
        .body(Body::empty())
        .unwrap();

    app.oneshot(request).await.unwrap();

    let logs = String::from_utf8(writer.0.lock().unwrap().clone()).unwrap();
    assert!(
        !logs.is_empty(),
        "trace regression must capture a real request log"
    );
    assert!(logs.contains("GET"));
    assert!(logs.contains("/api/v1/events/stream"));
    assert!(!logs.contains(MARKER));
    assert!(!logs.contains("?token="));
}
