use cronometrix_api::isapi::parser::{parse_buffer, parse_line_scan_fallback};

fn part(content_type: &str, bytes: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "--edge-boundary\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
        bytes.len()
    )
    .into_bytes();
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\r\n");
    out
}

#[tokio::test]
async fn multipart_magic_bytes_pair_consecutive_xml_and_drop_orphan_parts() {
    let xml_one = b"<EventNotificationAlert><eventType>one</eventType></EventNotificationAlert>";
    let xml_two = b"<EventNotificationAlert><eventType>two</eventType></EventNotificationAlert>";
    let jpeg = b"\xFF\xD8\xFFjpeg";
    let mut body = part("application/octet-stream", jpeg);
    body.extend(part("text/plain", xml_one));
    body.extend(part("text/plain", xml_two));
    body.extend(part("application/octet-stream", jpeg));
    body.extend(part("text/plain", b"ignored"));
    body.extend_from_slice(b"--edge-boundary--\r\n");

    let pairs = parse_buffer(&body, "edge-boundary").await.unwrap();
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].xml.as_ref(), xml_one);
    assert!(pairs[0].jpeg.is_none());
    assert_eq!(pairs[1].xml.as_ref(), xml_two);
    assert_eq!(pairs[1].jpeg.as_deref(), Some(jpeg.as_slice()));
}

#[test]
fn fallback_stops_at_an_unclosed_xml_and_handles_short_garbage() {
    assert!(parse_line_scan_fallback(b"<EventNotificationAlert><eventType>open").is_empty());
    assert!(parse_line_scan_fallback(b"short").is_empty());
}

#[test]
fn fallback_bounds_each_jpeg_at_the_next_xml_event() {
    let body = b"prefix<EventNotificationAlert>one</EventNotificationAlert>\
                 \xFF\xD8\xFFone-jpeg\
                 <EventNotificationAlert>two</EventNotificationAlert>suffix";
    let pairs = parse_line_scan_fallback(body);
    assert_eq!(pairs.len(), 2);
    assert!(pairs[0].jpeg.as_ref().unwrap().ends_with(b"one-jpeg"));
    assert!(pairs[1].jpeg.is_none());
}
