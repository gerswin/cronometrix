//! Phase 7 — Kiosk capture (D-02) integration tests (Wave 0 scaffolds).
//!
//! Covers the 2-step capture state machine: POST capture-from-device → 202 capture_id,
//! GET captures/:id polls until status=="captured" with photo_b64 inline.
//! Also covers the 30s timeout path and the photo_b64 base64 inline contract
//! reconciled with 07-02 Task 3 (kiosk-capture-tab.tsx).
//!
//! Populated in Tasks 4 and 6.

mod common;

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 6"]
async fn test_capture_from_device_returns_capture_id_immediately() {
    todo!("implement after Task 6 capture_from_device handler wired in main.rs")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 6"]
async fn test_get_capture_returns_jpg_bytes_after_device_responds() {
    todo!("implement after Task 6 get_capture handler wired + CapturedFacePicture wiremock")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 6"]
async fn test_capture_from_device_30s_timeout_yields_terminal_error() {
    todo!("implement after Task 6 capture_from_device timeout path wired")
}

/// Key contract test: when status=="captured", get_capture must return photo_b64
/// as a non-empty base64-encoded JPEG (Option<String> = Some(...)).
/// When status=="capturing" (before the device responds), photo_b64 must be absent
/// from the JSON response (skip_serializing_if = "Option::is_none").
///
/// Frontend kiosk-capture-tab.tsx (07-02 Task 3) decodes via:
///   atob(photo_b64) -> Uint8Array -> Blob -> URL.createObjectURL for preview.
#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 4"]
async fn test_get_capture_inlines_photo_b64_when_status_captured() {
    todo!("implement after Task 4 get_capture handler + base64 inline logic lands")
}
