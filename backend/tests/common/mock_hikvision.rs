//! Wave 0 test fixture: minimal tokio TCP server that mimics a Hikvision
//! alertStream endpoint by serving a canned multipart/mixed body after the
//! HTTP/1.1 200 response line. Plan 02-03 extends this helper with:
//!   - digest auth (401 -> authed 200)
//!   - mid-stream delays (reconnect/backoff testing)
//!   - multi-connection scenarios
//!
//! The plain variant in this module is deliberately minimal so Plan 02-02
//! can verify fixture bytes and the mock topology without dragging parser
//! work forward.

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
