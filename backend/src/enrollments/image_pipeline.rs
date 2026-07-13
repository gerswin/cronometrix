//! Server-side JPEG normalisation and downscale pipeline.
//!
//! D-04 SUPERSEDED (RESEARCH Lock-In): phone uploads (1–4 MB) exceed the Hikvision
//! 200 KB face-image cap. This module decodes, validates, and iteratively downscales
//! input JEPGs to ≤200 KB before they are persisted and pushed to devices.
//!
//! All operations are synchronous CPU work; callers MUST invoke `normalize_face_jpeg`
//! inside `tokio::task::spawn_blocking` to avoid blocking the async runtime.

use image::{codecs::jpeg::JpegEncoder, DynamicImage};

/// Maximum permitted JPEG output size (Hikvision face-profile cap per RESEARCH).
pub const MAX_FACE_BYTES: usize = 200 * 1024; // 200 KB

/// Target long-edge dimension for the first resize pass.
const TARGET_DIM_PX: u32 = 480;

/// Normalise an input JPEG for Hikvision face enrollment.
///
/// Steps:
/// 1. Validate JPEG magic bytes (reject PNG, HEIC, mislabelled files).
/// 2. Decode to `DynamicImage`.
/// 3. If ≤200 KB: re-encode at quality 90 to canonicalise the byte stream.
/// 4. If >200 KB: iteratively resize + re-encode until ≤200 KB:
///    - Pass 1: resize long edge to 480px, quality 85
///    - Pass 2: same size, quality 70
///    - Pass 3: resize long edge to 320px, quality 70
///    - Pass 4: resize long edge to 320px, quality 55
///    - After 4 passes still >200 KB: return an error (input is degenerate).
///
/// Returns the normalised JPEG bytes (≤200 KB).
///
/// # Errors
/// - `"not a JPEG"` — magic bytes mismatch.
/// - `"failed to decode JPEG"` — corrupt image data.
/// - `"image too large after 4 normalisation passes"` — extremely dense input.
pub fn normalize_face_jpeg(input: &[u8]) -> anyhow::Result<Vec<u8>> {
    // Magic byte check: JPEG starts with FF D8 FF.
    if input.len() < 3 || &input[..3] != &[0xFF, 0xD8, 0xFF] {
        anyhow::bail!(
            "not a JPEG: magic bytes mismatch (got {:02X?})",
            &input[..input.len().min(3)]
        );
    }

    // Decode once.
    let img = image::load_from_memory_with_format(input, image::ImageFormat::Jpeg)
        .map_err(|e| anyhow::anyhow!("failed to decode JPEG: {e}"))?;

    // Small image — just re-encode to canonicalise.
    if input.len() <= MAX_FACE_BYTES {
        return reencode_jpeg(&img, 90);
    }

    // Iterative downscale passes.
    let passes: &[(Option<u32>, u8)] = &[
        (Some(TARGET_DIM_PX), 85), // pass 1: 480px, q85
        (Some(TARGET_DIM_PX), 70), // pass 2: 480px, q70
        (Some(320), 70),           // pass 3: 320px, q70
        (Some(320), 55),           // pass 4: 320px, q55
    ];

    let mut current_img = img;
    for &(maybe_dim, quality) in passes {
        if let Some(dim) = maybe_dim {
            current_img = current_img.resize(dim, dim, image::imageops::FilterType::Lanczos3);
        }
        let encoded = reencode_jpeg(&current_img, quality)?;
        if encoded.len() <= MAX_FACE_BYTES {
            return Ok(encoded);
        }
    }

    anyhow::bail!(
        "image too large after 4 normalisation passes (final size {} bytes; max {} bytes)",
        {
            // Re-encode at the last pass params to get the size for the error message.
            reencode_jpeg(&current_img, 55)
                .map(|b| b.len())
                .unwrap_or(0)
        },
        MAX_FACE_BYTES
    )
}

/// Re-encode a `DynamicImage` as JPEG at the given quality (1–100).
/// Uses `JpegEncoder::new_with_quality` — the only public API that exposes quality.
fn reencode_jpeg(img: &DynamicImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut buf, quality)
        .encode_image(img)
        .map_err(|e| anyhow::anyhow!("JPEG encode failed at quality {quality}: {e}"))?;
    Ok(buf.into_inner())
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_jpeg(width: u32, height: u32, quality: u8) -> Vec<u8> {
        use image::{ImageBuffer, Rgb};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(width, height, |x, y| Rgb([x as u8, y as u8, 128u8]));
        let dynamic = DynamicImage::ImageRgb8(img);
        let mut buf = std::io::Cursor::new(Vec::new());
        JpegEncoder::new_with_quality(&mut buf, quality)
            .encode_image(&dynamic)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn rejects_non_jpeg_magic_bytes() {
        // PNG magic bytes
        let png = b"\x89PNG\r\n\x1a\n";
        let err = normalize_face_jpeg(png).unwrap_err();
        assert!(err.to_string().contains("not a JPEG"), "got: {err}");
    }

    #[test]
    fn passes_through_50kb_with_reencode_canonicalization() {
        let jpeg = make_jpeg(100, 100, 90);
        assert!(
            jpeg.len() <= MAX_FACE_BYTES,
            "fixture must be ≤200KB, got {}",
            jpeg.len()
        );
        let result = normalize_face_jpeg(&jpeg).unwrap();
        // Output is valid JPEG
        assert_eq!(&result[..3], &[0xFF, 0xD8, 0xFF]);
        assert!(result.len() <= MAX_FACE_BYTES);
    }

    #[test]
    fn downscales_4mb_to_under_200kb() {
        let jpeg = make_jpeg(2000, 2000, 95);
        assert!(
            jpeg.len() > MAX_FACE_BYTES,
            "fixture must be >200KB, got {}",
            jpeg.len()
        );
        let result = normalize_face_jpeg(&jpeg).unwrap();
        assert!(
            result.len() <= MAX_FACE_BYTES,
            "normalised result must be ≤200KB, got {} bytes",
            result.len()
        );
        // Output is valid JPEG
        assert_eq!(&result[..3], &[0xFF, 0xD8, 0xFF]);
    }
}
