//! Multipart parsing for every buffered payload a Hikvision reader sends.
//!
//! The device family emits `multipart/*` in three places — the alertStream, the
//! event webhook, and the `CaptureFaceData` response — and is inconsistent about
//! honouring RFC 7578 in each. Observed on DS-K1T341CMFW firmware V3.3.8:
//!
//! - the webhook omits the blank line between a part's headers and its body,
//!   and gives the event part no `Content-Type` at all;
//! - `CaptureFaceData` declares its boundary INSIDE the payload rather than in
//!   the HTTP `Content-Type` header;
//! - some units emit parts with no headers whatsoever (RESEARCH § Pitfall 2).
//!
//! A strict parser answers each of these by silently discarding the part, which
//! is how a reader can appear healthy while delivering nothing. This module is
//! therefore deliberately permissive, and is the single place to add the next
//! firmware quirk.
//!
//! `stream.rs` still drives `multer` directly: it consumes a live connection
//! rather than a buffer, and its parts have always been well formed.

use bytes::Bytes;

/// One part of a multipart body.
pub struct Part {
    /// Raw header block, lowercased for matching. Empty when the part had none.
    pub headers: String,
    pub body: Bytes,
}

impl Part {
    /// Whether this part carries a JPEG, by declared type or by magic bytes.
    ///
    /// The magic-byte check is not redundant: the webhook labels its image part
    /// correctly, but parts with no headers at all have been observed, and an
    /// image identified only by `Content-Disposition` would otherwise be taken
    /// for the event payload.
    pub fn is_jpeg(&self) -> bool {
        self.headers.contains("image/jpeg") || self.body.starts_with(&[0xFF, 0xD8, 0xFF])
    }

    /// Whether this part looks like an event payload rather than an attachment.
    pub fn is_alert(&self) -> bool {
        !self.is_jpeg()
            && self
                .body
                .iter()
                .find(|byte| !byte.is_ascii_whitespace())
                .is_some_and(|byte| matches!(byte, b'{' | b'<'))
    }

    /// The content type to hand a deserializer, sniffed when the device did not
    /// declare one.
    pub fn alert_content_type(&self) -> &'static str {
        if self.headers.contains("json") {
            return "application/json";
        }
        if self.headers.contains("xml") {
            return "application/xml";
        }
        match self.body.iter().find(|byte| !byte.is_ascii_whitespace()) {
            Some(b'{') => "application/json",
            _ => "application/xml",
        }
    }
}

/// One parsed event: the alert payload and its optional JPEG attachment.
///
/// Named `xml` for schema compatibility; on current firmware it holds JSON.
#[derive(Debug)]
pub struct EventPair {
    pub xml: Bytes,
    pub jpeg: Option<Bytes>,
}

/// Read the `boundary=` parameter out of a `multipart/*` content type.
///
/// `multer::parse_boundary` only accepts `multipart/form-data`; readers use both
/// that and `multipart/mixed`, so any `multipart/*` subtype is accepted here.
pub fn boundary_of(content_type: &str) -> Option<String> {
    let lower = content_type.to_ascii_lowercase();
    if !lower.starts_with("multipart/") {
        return None;
    }
    let index = lower.find("boundary=")?;
    let value = content_type[index + "boundary=".len()..]
        .split(';')
        .next()?
        .trim()
        .trim_matches('"')
        .to_string();
    (!value.is_empty()).then_some(value)
}

/// Read a boundary that the device declared inside the payload itself.
///
/// `CaptureFaceData` responds with the multipart framing in the body's opening
/// lines rather than in the HTTP header, so the boundary has to be recovered
/// from the bytes. Bounded to the first 512 bytes so a large JPEG is never
/// lossily converted just to find it.
pub fn boundary_in_body(body: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&body[..body.len().min(512)]);
    boundary_of(head.lines().next()?.trim_start_matches("Content-Type:").trim())
        .or_else(|| {
            let index = head.find("boundary=")?;
            let value = head[index + "boundary=".len()..]
                .split(['\r', '\n', ';'])
                .next()?
                .trim()
                .trim_matches('"')
                .to_string();
            (!value.is_empty()).then_some(value)
        })
}

/// Split a buffered multipart body into its parts.
///
/// Tolerant by design — see the module docs for what each firmware omits.
pub fn split_multipart(body: &[u8], boundary: &str) -> Vec<Part> {
    let delimiter = format!("--{boundary}");
    let mut parts = Vec::new();
    let mut cursor = 0usize;

    while let Some(offset) = find_subslice(&body[cursor..], delimiter.as_bytes()) {
        let start = cursor + offset + delimiter.len();
        let end = find_subslice(&body[start..], delimiter.as_bytes())
            .map(|next| start + next)
            .unwrap_or(body.len());
        cursor = end;

        let segment = &body[start..end];
        let (headers, content_start) = split_headers(segment);
        let mut content = &segment[content_start..];
        // Trim the CRLF that terminates a part body, and the `--` of a closing
        // delimiter when this was the final part.
        while content
            .last()
            .is_some_and(|byte| matches!(byte, b'\r' | b'\n' | b'-'))
        {
            content = &content[..content.len() - 1];
        }
        if content.is_empty() {
            continue;
        }
        parts.push(Part {
            headers: headers.to_ascii_lowercase(),
            body: Bytes::copy_from_slice(content),
        });

        if end == body.len() {
            break;
        }
    }

    parts
}

/// Pair each alert part with the JPEG that follows it.
///
/// An alert with no attachment still yields a pair rather than being buffered
/// indefinitely: firmware that sends no picture is the common case, not an
/// error. An orphan image with no preceding alert is dropped.
pub fn pair_events(parts: Vec<Part>) -> Vec<EventPair> {
    let mut out = Vec::new();
    let mut pending: Option<Bytes> = None;

    for part in parts {
        if part.is_jpeg() {
            if let Some(alert) = pending.take() {
                out.push(EventPair {
                    xml: alert,
                    jpeg: Some(part.body),
                });
            }
        } else if part.is_alert() {
            if let Some(alert) = pending.take() {
                out.push(EventPair {
                    xml: alert,
                    jpeg: None,
                });
            }
            pending = Some(part.body);
        }
    }

    if let Some(alert) = pending {
        out.push(EventPair {
            xml: alert,
            jpeg: None,
        });
    }
    out
}

/// Parse a complete buffered body into `EventPair`s.
pub fn parse_buffer(body: &[u8], boundary: &str) -> Vec<EventPair> {
    pair_events(split_multipart(body, boundary))
}

/// Where a part's headers end and its body begins.
///
/// Returns `("", 0)` when the part has no headers at all. The fallback walks
/// CRLF-separated lines and stops at the first that is not `Name: value`,
/// because the webhook writes its `Content-Disposition` immediately before the
/// JSON with no blank line between them.
fn split_headers(segment: &[u8]) -> (&str, usize) {
    let mut cursor = 0usize;
    while segment[cursor..].starts_with(b"\r\n") {
        cursor += 2;
    }
    if cursor >= segment.len() {
        return ("", segment.len());
    }

    // A part that opens straight into a payload has no headers.
    if matches!(segment[cursor], b'{' | b'<' | 0xFF) {
        return ("", cursor);
    }

    if let Some(index) = find_subslice(&segment[cursor..], b"\r\n\r\n") {
        let end = cursor + index;
        return (
            std::str::from_utf8(&segment[cursor..end]).unwrap_or_default(),
            end + 4,
        );
    }

    let mut scan = cursor;
    while scan < segment.len() {
        let Some(relative) = find_subslice(&segment[scan..], b"\r\n") else {
            break;
        };
        let line_end = scan + relative;
        if !is_header_line(&segment[scan..line_end]) {
            return (
                std::str::from_utf8(&segment[cursor..scan.saturating_sub(2)]).unwrap_or_default(),
                scan,
            );
        }
        scan = line_end + 2;
    }
    ("", cursor)
}

/// Whether a line is a MIME header rather than the start of a payload.
///
/// The name must be alphanumeric-or-dash: a JSON body is full of colons, and
/// mistaking its first line for a header would swallow part of the event.
fn is_header_line(line: &[u8]) -> bool {
    let Some(colon) = line.iter().position(|byte| *byte == b':') else {
        return false;
    };
    let name = &line[..colon];
    !name.is_empty()
        && name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

/// Last-resort parser for payloads with no usable multipart framing at all.
///
/// Scans for `<EventNotificationAlert>` marker pairs and, after each close tag,
/// for a JPEG SOI up to the next opening tag. XML-only by construction — it
/// predates the JSON firmware and does NOT recover JSON payloads. Deliberately
/// permissive: one extra trailing byte on an image beats dropping an event.
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

        let tail_start = abs_c;
        let tail_end = find_subslice(&body[tail_start..], OPEN)
            .map(|n| tail_start + n)
            .unwrap_or(body.len());

        let jpeg = find_subslice(&body[tail_start..tail_end], b"\xFF\xD8\xFF")
            .map(|j| Bytes::copy_from_slice(&body[tail_start + j..tail_end]));

        out.push(EventPair {
            xml: xml_bytes,
            jpeg,
        });
        start = tail_end;
    }

    out
}

/// Locate `needle` inside `haystack`, returning its start offset.
pub fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(headers: &str, body: &[u8]) -> Vec<u8> {
        let mut out = format!("--B\r\n{headers}\r\n\r\n").into_bytes();
        out.extend_from_slice(body);
        out.extend_from_slice(b"\r\n");
        out
    }

    const ALERT_XML: &[u8] = b"<EventNotificationAlert><eventType>x</eventType></EventNotificationAlert>";
    const ALERT_JSON: &[u8] = br#"{"eventType":"AccessControllerEvent"}"#;
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0x00, 0xFF, 0xD9];

    #[test]
    fn pairs_an_alert_with_the_image_that_follows_it() {
        let mut body = part("Content-Type: application/xml", ALERT_XML);
        body.extend(part("Content-Type: image/jpeg", JPEG));
        body.extend_from_slice(b"--B--\r\n");

        let pairs = parse_buffer(&body, "B");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].xml, Bytes::from_static(ALERT_XML));
        assert_eq!(pairs[0].jpeg.as_deref(), Some(JPEG));
    }

    /// The webhook writes `Content-Disposition` immediately before the JSON with
    /// no blank line. Requiring one discarded every pushed marking while the
    /// handler still answered success.
    #[test]
    fn accepts_a_part_with_no_blank_line_before_the_body() {
        let mut body = b"--B\r\nContent-Disposition: form-data; name=\"event_log\"\r\n".to_vec();
        body.extend_from_slice(ALERT_JSON);
        body.extend_from_slice(b"\r\n--B--\r\n");

        let pairs = parse_buffer(&body, "B");
        assert_eq!(pairs.len(), 1, "the part must not be discarded");
        assert_eq!(pairs[0].xml, Bytes::from_static(ALERT_JSON));
    }

    /// Parts with no headers at all have been seen in the field.
    #[test]
    fn accepts_a_part_with_no_headers_at_all() {
        let mut body = b"--B\r\n".to_vec();
        body.extend_from_slice(ALERT_JSON);
        body.extend_from_slice(b"\r\n--B\r\n");
        body.extend_from_slice(JPEG);
        body.extend_from_slice(b"\r\n--B--\r\n");

        let pairs = parse_buffer(&body, "B");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].jpeg.as_deref(), Some(JPEG));
    }

    #[test]
    fn an_alert_without_an_image_still_yields_a_pair() {
        let mut body = part("Content-Type: application/xml", ALERT_XML);
        body.extend(part("Content-Type: application/xml", ALERT_XML));
        body.extend_from_slice(b"--B--\r\n");

        let pairs = parse_buffer(&body, "B");
        assert_eq!(pairs.len(), 2);
        assert!(pairs.iter().all(|pair| pair.jpeg.is_none()));
    }

    #[test]
    fn drops_an_image_with_no_preceding_alert() {
        let mut body = part("Content-Type: image/jpeg", JPEG);
        body.extend_from_slice(b"--B--\r\n");
        assert!(parse_buffer(&body, "B").is_empty());
    }

    #[test]
    fn ignores_bytes_before_the_first_boundary() {
        let mut body = b"preamble junk\r\n".to_vec();
        body.extend(part("Content-Type: application/xml", ALERT_XML));
        body.extend_from_slice(b"--B--\r\n");
        assert_eq!(parse_buffer(&body, "B").len(), 1);
    }

    #[test]
    fn sniffs_the_payload_format_when_the_part_declares_none() {
        let parts = split_multipart(
            &[
                b"--B\r\nContent-Disposition: form-data\r\n".as_slice(),
                ALERT_JSON,
                b"\r\n--B--\r\n",
            ]
            .concat(),
            "B",
        );
        assert_eq!(parts[0].alert_content_type(), "application/json");
    }

    /// `is_alert` must never fire for a part that is declared `image/jpeg`,
    /// even when its body happens to start with `{` — `is_jpeg()` (true here
    /// via the declared header, not the magic bytes) short-circuits the rest
    /// of the check before the body is ever scanned.
    #[test]
    fn is_alert_is_false_for_a_jpeg_part_even_when_the_body_looks_like_json() {
        let part = Part {
            headers: "content-type: image/jpeg".to_string(),
            body: Bytes::from_static(b"{\"not\":\"actually a jpeg body\"}"),
        };
        assert!(part.is_jpeg(), "declared image/jpeg must be enough on its own");
        assert!(!part.is_alert());
    }

    /// A declared `application/xml` type must win over sniffing, even when
    /// the body's first non-whitespace byte would sniff as JSON.
    #[test]
    fn alert_content_type_uses_the_declared_xml_type_over_sniffing() {
        let part = Part {
            headers: "content-type: application/xml".to_string(),
            body: Bytes::from_static(b"{not actually json}"),
        };
        assert_eq!(part.alert_content_type(), "application/xml");
    }

    /// Some firmware drops the connection right after the last part's body
    /// and never sends the terminating `--B--`. The part must still survive.
    #[test]
    fn split_multipart_recovers_the_last_part_when_the_stream_ends_without_a_closing_delimiter() {
        let mut body = b"--B\r\nContent-Type: application/xml\r\n\r\n".to_vec();
        body.extend_from_slice(ALERT_XML);
        // Deliberately no trailing CRLF and no closing "--B--".

        let parts = split_multipart(&body, "B");
        assert_eq!(parts.len(), 1, "the unterminated final part must not be dropped");
        assert_eq!(parts[0].body.as_ref(), ALERT_XML);
    }

    #[test]
    fn boundary_of_rejects_an_empty_boundary_value() {
        assert_eq!(boundary_of("multipart/mixed; boundary="), None);
    }

    #[test]
    fn boundary_of_accepts_any_multipart_subtype() {
        assert_eq!(
            boundary_of("multipart/mixed; boundary=MIME_boundary").as_deref(),
            Some("MIME_boundary")
        );
        assert_eq!(
            boundary_of("multipart/form-data; boundary=\"quoted\"").as_deref(),
            Some("quoted")
        );
        assert_eq!(boundary_of("application/json"), None);
        assert_eq!(boundary_of("multipart/mixed"), None);
    }

    /// `CaptureFaceData` declares its boundary in the payload, not the header.
    #[test]
    fn boundary_in_body_reads_the_payloads_own_declaration() {
        let body = b"Content-Type:multipart/form-data;boundary=MIME_boundary\r\nContent-Length:12\r\n\r\n--MIME_boundary\r\n";
        assert_eq!(boundary_in_body(body).as_deref(), Some("MIME_boundary"));
    }

    #[test]
    fn line_scan_fallback_recovers_events_without_any_framing() {
        let mut body = ALERT_XML.to_vec();
        body.extend_from_slice(JPEG);
        body.extend_from_slice(ALERT_XML);

        let pairs = parse_line_scan_fallback(&body);
        assert_eq!(pairs.len(), 2);
        assert!(pairs[0].jpeg.is_some(), "first event keeps its image");
        assert!(pairs[1].jpeg.is_none(), "second has none to claim");
    }

    #[test]
    fn line_scan_fallback_stops_at_an_unclosed_alert() {
        assert!(parse_line_scan_fallback(b"<EventNotificationAlert>truncated").is_empty());
        assert!(parse_line_scan_fallback(b"garbage").is_empty());
    }

    #[test]
    fn split_headers_of_an_empty_segment_yields_no_headers() {
        assert_eq!(split_headers(b""), ("", 0));
        assert_eq!(split_headers(b"\r\n"), ("", 2));
    }

    /// A segment whose bytes all look like header lines, right up to the end,
    /// with no blank-line terminator anywhere. The parser must give up and
    /// hand the bytes back as unparsed content instead of quietly treating
    /// them as headers and losing them.
    #[test]
    fn split_headers_gives_up_when_every_line_looks_like_a_header_but_none_ends_the_block() {
        let segment = b"X-Foo: bar\r\nX-Baz: qux\r\n";
        let (headers, start) = split_headers(segment);
        assert_eq!(headers, "");
        assert_eq!(start, 0, "unterminated headers must fall back to treating the segment as body");
    }

    /// A line that never reaches a CRLF at all (the segment is cut off
    /// mid-line). Same fallback as the fully-header-shaped case above.
    #[test]
    fn split_headers_gives_up_when_a_line_never_reaches_a_crlf() {
        let segment = b"X-Foo: bar\r\nno-terminator-here";
        let (headers, start) = split_headers(segment);
        assert_eq!(headers, "");
        assert_eq!(start, 0);
    }

    #[test]
    fn is_header_line_rejects_a_bare_colon_with_no_name() {
        assert!(!is_header_line(b":no-name"));
    }

    #[test]
    fn find_subslice_with_an_empty_needle_returns_none() {
        assert_eq!(find_subslice(b"anything", b""), None);
    }
}
