//! Multipart alertStream parser (Plan 02-03 Task 1).
//!
//! Hikvision's `/ISAPI/Event/notification/alertStream` serves
//! `multipart/mixed` with XML and JPEG parts interleaved. `multer` handles the
//! common case (RFC 7578 parts with `Content-Type` headers); if a device
//! firmware emits non-standard parts (seen on some K1T342 units — RESEARCH
//! § Pitfall 2) we fall back to a byte-level scan for the
//! `<EventNotificationAlert>` / `</EventNotificationAlert>` markers.
//!
//! The parser MUST preserve pair ordering: XML is always emitted before its
//! JPEG attachment, and an XML part without a following JPEG must still yield
//! an EventPair (jpeg=None) rather than buffering indefinitely.

use bytes::Bytes;

/// One parsed event: the XML block and its optional JPEG attachment.
#[derive(Debug)]
pub struct EventPair {
    pub xml: Bytes,
    pub jpeg: Option<Bytes>,
}

/// T-2-19 mitigation — bound per-part and whole-stream memory so a hostile
/// device (or MITM tampering pre-TLS) cannot force unbounded allocation.
/// Per-field 10 MB is well above realistic JPEG sizes from K1T341/K1T342
/// (typical capture ≤150 KB). Whole-stream 64 MB is only applied when
/// parsing a single buffered body; the live stream path in `stream.rs` sets
/// `whole_stream = 1 GiB` because the connection is long-lived.
const PER_FIELD_LIMIT: u64 = 10 * 1024 * 1024;
const BUFFER_WHOLE_LIMIT: u64 = 64 * 1024 * 1024;

/// Parse a complete buffered body into `EventPair`s using `multer`.
///
/// Used by unit tests driving fixture bytes through the parser. For live
/// streams we instantiate `multer::Multipart` directly against
/// `reqwest::Response::bytes_stream()` inside `stream::connect_and_stream`.
pub async fn parse_buffer(body: &[u8], boundary: &str) -> anyhow::Result<Vec<EventPair>> {
    let owned = Bytes::copy_from_slice(body);
    // `multer` needs a Stream of Result<Bytes, E>; we hand it one chunk.
    let stream = futures::stream::once(async move { Ok::<_, std::io::Error>(owned) });

    let constraints = multer::Constraints::new().size_limit(
        multer::SizeLimit::new()
            .per_field(PER_FIELD_LIMIT)
            .whole_stream(BUFFER_WHOLE_LIMIT),
    );
    let mut mp = multer::Multipart::with_constraints(stream, boundary, constraints);

    let mut out = Vec::new();
    let mut pending_xml: Option<Bytes> = None;

    while let Some(field) = mp.next_field().await? {
        let ct = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_default();
        let bytes = field.bytes().await?;

        if ct.starts_with("application/xml") || bytes.starts_with(b"<EventNotificationAlert") {
            // Commit any pending XML with no JPEG (Pitfall 2 — some streams
            // omit the JPEG part entirely).
            if let Some(prev) = pending_xml.take() {
                out.push(EventPair { xml: prev, jpeg: None });
            }
            pending_xml = Some(bytes);
        } else if ct.starts_with("image/jpeg") || bytes.starts_with(b"\xFF\xD8\xFF") {
            if let Some(prev) = pending_xml.take() {
                out.push(EventPair { xml: prev, jpeg: Some(bytes) });
            }
            // else: orphan image with no preceding XML — drop it.
        }
    }

    if let Some(prev) = pending_xml.take() {
        out.push(EventPair { xml: prev, jpeg: None });
    }
    Ok(out)
}

/// Fallback parser for payloads that lack standard multipart part headers.
///
/// Scans the whole buffer for `<EventNotificationAlert>` /
/// `</EventNotificationAlert>` marker pairs and, immediately after each close
/// tag, searches for a JPEG SOI marker (`FF D8 FF`) up to the next
/// `<EventNotificationAlert` (or end-of-buffer). This is deliberately
/// permissive — we prefer "one extra JPEG byte" to "drop the whole event".
pub fn parse_line_scan_fallback(body: &[u8]) -> Vec<EventPair> {
    const OPEN: &[u8] = b"<EventNotificationAlert";
    const CLOSE: &[u8] = b"</EventNotificationAlert>";

    let mut out = Vec::new();
    let mut start = 0usize;

    while let Some(o_rel) = find_subslice(&body[start..], OPEN) {
        let abs_o = start + o_rel;
        let Some(c_rel) = find_subslice(&body[abs_o..], CLOSE) else {
            break;
        };
        let abs_c = abs_o + c_rel + CLOSE.len();
        let xml_bytes = Bytes::copy_from_slice(&body[abs_o..abs_c]);

        // Look for a JPEG SOI between the close tag and the next opening tag (or EOF).
        let tail_start = abs_c;
        let tail_end = find_subslice(&body[tail_start..], OPEN)
            .map(|n| tail_start + n)
            .unwrap_or(body.len());

        let jpeg = find_subslice(&body[tail_start..tail_end], b"\xFF\xD8\xFF")
            .map(|j| Bytes::copy_from_slice(&body[tail_start + j..tail_end]));

        out.push(EventPair { xml: xml_bytes, jpeg });
        start = tail_end;
    }

    out
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

// =============================================================================
// Unit tests
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture_path(name: &str) -> String {
        format!("tests/fixtures/{}", name)
    }

    #[tokio::test]
    async fn parses_k1t341_fixture_into_one_event_pair() {
        let body = fs::read(fixture_path("alertstream_k1t341.bin")).expect("fixture missing");
        let pairs = parse_buffer(&body, "MIME_boundary").await.expect("parse");
        assert_eq!(pairs.len(), 1, "k1t341 fixture must yield exactly one pair");
        let pair = &pairs[0];
        let xml = std::str::from_utf8(&pair.xml).expect("xml utf8");
        assert!(xml.contains("<EventNotificationAlert"));
        assert!(pair.jpeg.is_some(), "k1t341 fixture has an attached JPEG");
        let jpeg = pair.jpeg.as_ref().unwrap();
        assert!(jpeg.starts_with(b"\xFF\xD8\xFF"), "JPEG must start with SOI");
    }

    #[tokio::test]
    async fn parses_heartbeat_fixture_into_xml_only_pair() {
        let body = fs::read(fixture_path("alertstream_heartbeat.bin")).expect("fixture missing");
        let pairs = parse_buffer(&body, "MIME_boundary").await.expect("parse");
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].jpeg.is_none(), "heartbeat fixture has no JPEG");
    }

    #[tokio::test]
    async fn parses_unknown_face_fixture_into_one_event_pair() {
        let body =
            fs::read(fixture_path("alertstream_unknown_face.bin")).expect("fixture missing");
        let pairs = parse_buffer(&body, "MIME_boundary").await.expect("parse");
        assert_eq!(pairs.len(), 1);
        let xml = std::str::from_utf8(&pairs[0].xml).expect("xml utf8");
        assert!(xml.contains("<faceID>"));
    }

    #[tokio::test]
    async fn ignores_bytes_before_first_boundary() {
        let fixture =
            fs::read(fixture_path("alertstream_k1t341.bin")).expect("fixture missing");
        let mut body = vec![0u8; 128];
        body.extend_from_slice(&fixture);
        let pairs = parse_buffer(&body, "MIME_boundary").await.expect("parse");
        assert_eq!(pairs.len(), 1, "junk prefix must not change pair count");
    }

    #[tokio::test]
    async fn fallback_line_scan_if_multer_fails() {
        // Build a body with the XML and JPEG but WITHOUT Content-Disposition /
        // Content-Type headers — just raw bytes separated by the boundary.
        // multer should yield zero parts (missing headers), while the line-scan
        // fallback should recover one EventPair.
        let xml = r#"<EventNotificationAlert version="2.0"><eventType>AccessControllerEvent</eventType></EventNotificationAlert>"#;
        let jpeg: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, b'J', b'F', b'I', b'F', 0xFF, 0xD9];

        let mut body = Vec::new();
        body.extend_from_slice(b"--MIME_boundary\r\n\r\n");
        body.extend_from_slice(xml.as_bytes());
        body.extend_from_slice(b"\r\n--MIME_boundary\r\n\r\n");
        body.extend_from_slice(jpeg);
        body.extend_from_slice(b"\r\n--MIME_boundary--\r\n");

        // Primary parser may still succeed here because multer sometimes tolerates
        // missing Content-Type. What we actually pin is that the fallback alone
        // can reconstruct the pair.
        let fallback_pairs = parse_line_scan_fallback(&body);
        assert_eq!(fallback_pairs.len(), 1, "fallback must recover one pair");
        let pair = &fallback_pairs[0];
        let recovered_xml = std::str::from_utf8(&pair.xml).unwrap();
        assert!(recovered_xml.contains("<EventNotificationAlert"));
        assert!(recovered_xml.contains("</EventNotificationAlert>"));
        assert!(pair.jpeg.is_some(), "fallback must find the JPEG SOI");
    }

    #[test]
    fn fallback_handles_multiple_events_in_same_buffer() {
        let xml1 = b"<EventNotificationAlert><a>1</a></EventNotificationAlert>";
        let xml2 = b"<EventNotificationAlert><a>2</a></EventNotificationAlert>";
        let mut body = Vec::new();
        body.extend_from_slice(xml1);
        body.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x01]);
        body.extend_from_slice(xml2);
        body.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x02]);

        let pairs = parse_line_scan_fallback(&body);
        assert_eq!(pairs.len(), 2);
        assert!(pairs[0].jpeg.is_some());
        assert!(pairs[1].jpeg.is_some());
    }

    #[test]
    fn fallback_returns_empty_for_garbage() {
        let pairs = parse_line_scan_fallback(b"no alerts here at all");
        assert!(pairs.is_empty());
    }
}
