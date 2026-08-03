//! Shared evidence-upload magic-byte sniffing.
//!
//! CR-03 / M-07 mitigation: the client-supplied multipart `Content-Type`
//! header is advisory only — it is trivially spoofable (e.g. uploading an
//! HTML payload declared as `image/jpeg`). The authoritative type, and the
//! extension under which the file is persisted, MUST be derived from the
//! file's own magic bytes. Every evidence-upload handler (daily record
//! overrides, leaves) shares this single check so the accepted type list
//! cannot drift between call sites.

/// Derives the canonical evidence file extension from magic bytes rather
/// than the client-supplied multipart Content-Type. Returns the canonical
/// extension (`pdf`, `jpg`, `png`) when the bytes start with a known
/// signature, otherwise `None`.
pub fn infer_evidence_ext_from_magic(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"%PDF") {
        return Some("pdf");
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some("jpg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pdf_magic() {
        assert_eq!(infer_evidence_ext_from_magic(b"%PDF-1.4 rest"), Some("pdf"));
    }

    #[test]
    fn detects_jpeg_magic() {
        assert_eq!(
            infer_evidence_ext_from_magic(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some("jpg")
        );
    }

    #[test]
    fn detects_png_magic() {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(b"rest of file");
        assert_eq!(infer_evidence_ext_from_magic(&bytes), Some("png"));
    }

    #[test]
    fn rejects_unknown_magic() {
        assert_eq!(infer_evidence_ext_from_magic(b"<html><body>hi"), None);
    }

    #[test]
    fn rejects_short_buffers() {
        assert_eq!(infer_evidence_ext_from_magic(&[0xFF]), None);
        assert_eq!(infer_evidence_ext_from_magic(b""), None);
    }
}
