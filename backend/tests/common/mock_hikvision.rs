//! Wave 0 test fixture: minimal tokio TCP server that mimics a Hikvision
//! alertStream endpoint by serving a canned multipart/mixed body after the
//! HTTP/1.1 200 response line. Plan 02-03 extends this helper with:
//!   - digest auth (401 -> authed 200) — `spawn_mock_hikvision_digest`
//!   - always-401 error path — `spawn_mock_hikvision_401`
//!
//! The plain variant in this module remains available for simple tests that
//! don't need the digest challenge cycle.

use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawns a tokio task that accepts ONE TCP connection and serves the given
/// multipart body with the given boundary. Returns the bound ephemeral address.
///
/// The fixture does NOT require digest auth — it simply writes a 200 response
/// followed by the canned bytes. Plan 02-03 introduces a digest-enforcing
/// variant in the same module.
pub async fn spawn_mock_hikvision_plain(body: Vec<u8>, boundary: &str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    let boundary = boundary.to_string();

    tokio::spawn(async move {
        if let Ok((mut sock, _)) = listener.accept().await {
            // Drain the request line + headers. 4 KB is plenty for a GET without a body.
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf).await;

            let response_head = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: multipart/mixed; boundary={}\r\n\
                 Connection: close\r\n\
                 Content-Length: {}\r\n\r\n",
                boundary,
                body.len()
            );
            let _ = sock.write_all(response_head.as_bytes()).await;
            let _ = sock.write_all(&body).await;
            let _ = sock.shutdown().await;
        }
    });

    addr
}

/// Digest-auth-enforcing mock. Implements the minimal RFC 2617 subset needed
/// for `diqwest` to complete a challenge cycle:
///   1. First request -> 401 with `WWW-Authenticate: Digest realm="..." nonce="..."`
///   2. Second request (with Authorization header) -> 200 + canned body
///
/// We do NOT validate the client's digest response hash — the goal is to
/// exercise the CHALLENGE cycle, not the crypto correctness (`diqwest`
/// upstream has its own unit tests for that). Any Authorization header on
/// request 2 is accepted.
///
/// Accepts up to 2 sequential connections per spawn so `connect_and_stream`
/// can reconnect once within the same test (diqwest opens a NEW connection
/// for the authed retry on some reqwest versions — close+retry is safer
/// than buffering).
pub async fn spawn_mock_hikvision_digest(
    body: Vec<u8>,
    boundary: &str,
    _username: &str,
    _password: &str,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    let boundary = boundary.to_string();

    tokio::spawn(async move {
        // diqwest issues the challenge retry on a FRESH connection after
        // the 401 closes the first one (the client's HTTP connection pool
        // re-establishes for the qop=auth retry). Handle up to 4 connects
        // so the test is tolerant of reqwest's pool semantics.
        for _ in 0..4 {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };

            // Read until we see the end-of-headers marker. For a GET with
            // digest auth headers, the full request is <2KB so a single
            // 8KB buffer is plenty, BUT we keep reading if the first read
            // doesn't contain "\r\n\r\n" (Linux TCP sometimes splits).
            let mut accumulated = Vec::new();
            loop {
                let mut chunk = [0u8; 4096];
                match sock.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => {
                        accumulated.extend_from_slice(&chunk[..n]);
                        if accumulated.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                        if accumulated.len() >= 16 * 1024 {
                            break; // safety — never buffer more than 16KB headers
                        }
                    }
                    Err(_) => break,
                }
            }

            // HTTP headers are case-insensitive; reqwest/hyper emits them
            // lowercase (`authorization:`). Match case-insensitively.
            let req_lower = String::from_utf8_lossy(&accumulated).to_ascii_lowercase();

            if req_lower.contains("authorization: digest") {
                let response_head = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: multipart/mixed; boundary={}\r\n\
                     Connection: close\r\n\
                     Content-Length: {}\r\n\r\n",
                    boundary,
                    body.len()
                );
                let _ = sock.write_all(response_head.as_bytes()).await;
                let _ = sock.write_all(&body).await;
                let _ = sock.shutdown().await;
                return;
            } else {
                // Use a FIXED nonce so diqwest can compute the response
                // deterministically. Any nonce value works — the mock does
                // not validate the client's MD5 hash. Keep the WWW-
                // Authenticate header minimal — some digest-auth crates
                // reject unknown directives.
                let challenge = "HTTP/1.1 401 Unauthorized\r\n\
                     WWW-Authenticate: Digest realm=\"Hikvision\", qop=\"auth\", nonce=\"0123456789abcdef\"\r\n\
                     Content-Length: 0\r\n\
                     Connection: close\r\n\r\n";
                let _ = sock.write_all(challenge.as_bytes()).await;
                let _ = sock.shutdown().await;
            }
        }
    });

    addr
}

/// Always-401 mock. Used for the `connect_and_stream_fails_cleanly_on_401`
/// test — returns a 401 without a digest challenge so `diqwest` exhausts
/// its retry cycle and bubbles the error back up.
pub async fn spawn_mock_hikvision_401(_username: &str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");

    tokio::spawn(async move {
        // Accept up to 2 connections — diqwest may retry once.
        for _ in 0..2 {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf).await;
            // 401 with a malformed/absent challenge so diqwest gives up.
            let resp = b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = sock.write_all(resp).await;
            let _ = sock.shutdown().await;
        }
    });

    addr
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::net::TcpStream;

    fn fixture_path(name: &str) -> String {
        // Tests run with the crate as the CWD, so paths are relative to backend/.
        format!("tests/fixtures/{}", name)
    }

    #[tokio::test]
    async fn mock_hikvision_serves_canned_body() {
        let body = b"--MIME_boundary\r\n\
                     Content-Type: application/xml\r\n\
                     Content-Length: 3\r\n\r\n\
                     xyz\r\n\
                     --MIME_boundary--\r\n"
            .to_vec();
        let addr = spawn_mock_hikvision_plain(body.clone(), "MIME_boundary").await;

        let mut stream = TcpStream::connect(addr).await.expect("connect");
        stream
            .write_all(b"GET /ISAPI/Event/notification/alertStream HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .expect("write request");

        let mut out = Vec::new();
        stream.read_to_end(&mut out).await.expect("read");

        let text = String::from_utf8_lossy(&out);
        assert!(text.starts_with("HTTP/1.1 200 OK"), "got: {}", text);
        assert!(text.contains("multipart/mixed; boundary=MIME_boundary"));
        // Body bytes follow the CRLFCRLF separator.
        let split = out
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("header/body split");
        let body_bytes = &out[split + 4..];
        assert_eq!(body_bytes, body.as_slice());
    }

    #[test]
    fn fixture_k1t341_exists_and_contains_event_xml() {
        const MARKER: &[u8] = b"<EventNotificationAlert";
        let bytes = fs::read(fixture_path("alertstream_k1t341.bin")).expect("fixture missing");
        assert!(
            bytes.windows(MARKER.len()).any(|w| w == MARKER),
            "alertstream_k1t341.bin must contain <EventNotificationAlert"
        );
    }

    #[test]
    fn fixture_heartbeat_exists_and_contains_heartbeat_marker() {
        let bytes = fs::read(fixture_path("alertstream_heartbeat.bin")).expect("fixture missing");
        let s = String::from_utf8_lossy(&bytes);
        assert!(
            s.contains("videoloss") || s.contains("Heartbeat"),
            "heartbeat fixture must contain 'videoloss' or 'Heartbeat' marker"
        );
    }

    #[test]
    fn fixture_unknown_face_has_face_id() {
        let bytes =
            fs::read(fixture_path("alertstream_unknown_face.bin")).expect("fixture missing");
        let s = String::from_utf8_lossy(&bytes);
        assert!(
            s.contains("<faceID>"),
            "unknown-face fixture must contain <faceID>"
        );
    }
}
