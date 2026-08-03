use axum::http::Request;
use tower_http::trace::{HttpMakeClassifier, MakeSpan, TraceLayer};
use tracing::Span;

/// Sustituye el token de un push por `[redacted]`.
///
/// C-08: la ruta lleva el secreto de escritura del device
/// (`/devices/{id}/push/{token}`). El firmware Hikvision no admite cabeceras
/// arbitrarias en `httpHosts`, así que el secreto tiene que seguir en la URI;
/// lo que no puede es sobrevivir en un log.
///
/// This layer is global (installed on the outermost Router, before
/// routing), so it must redact unconditionally once `/push/` is found —
/// there is no fallback arm that re-emits the original path once a token
/// was actually present. A trailing slash, a stray extra path segment
/// (proxy normalization, firmware quirk, a retry), or anything else after
/// the token gets a 404 at the route level but the span for it is built
/// first, so the previous "only redact if nothing follows the token"
/// guard could still leak the secret to the log on that request.
///
/// Only the first segment after `/push/` (the token) is replaced; any
/// further segments are kept byte-identical so this never silently
/// swallows observability for the rest of the path.
///
/// Note: this matches `/push/` as a plain substring anywhere in the path,
/// not anchored to the device push route specifically. Harmless today —
/// the device push endpoint is the only route containing `/push/` — but a
/// future route with `/push/` in its own path would also lose path
/// observability here; anchor this to the actual route pattern if that
/// ever happens.
pub fn redact_path(path: &str) -> String {
    let Some(marker_start) = path.find("/push/") else {
        return path.to_string();
    };
    let prefix = &path[..marker_start];
    let rest = &path[marker_start + "/push/".len()..];
    if rest.is_empty() {
        return path.to_string();
    }

    let mut redacted = format!("{prefix}/push/[redacted]");
    if let Some(next_slash) = rest.find('/') {
        redacted.push_str(&rest[next_slash..]);
    }
    redacted
}

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
            path = %redact_path(request.uri().path()),
        )
    }
}

pub fn layer() -> TraceLayer<HttpMakeClassifier, SafeMakeSpan> {
    TraceLayer::new_for_http().make_span_with(SafeMakeSpan)
}
