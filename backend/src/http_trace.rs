use axum::http::Request;
use tower_http::trace::{HttpMakeClassifier, MakeSpan, TraceLayer};
use tracing::Span;

/// Produces request spans that deliberately exclude URI query strings.
///
/// The SSE endpoint authenticates via `?token=...`; recording the full URI in
/// DEBUG/TRACE logs would therefore disclose a bearer credential.
#[derive(Debug, Clone, Copy, Default)]
pub struct SafeMakeSpan;

impl<B> MakeSpan<B> for SafeMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        tracing::debug_span!(
            "request",
            method = %request.method(),
            path = %request.uri().path(),
        )
    }
}

pub fn layer() -> TraceLayer<HttpMakeClassifier, SafeMakeSpan> {
    TraceLayer::new_for_http().make_span_with(SafeMakeSpan)
}
