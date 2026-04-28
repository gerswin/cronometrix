//! Helper functions that build Hikvision ISAPI request bodies for face operations.
//!
//! D-12 LOCKED: modern 2-step face profile push:
//!   Step 1 — POST /ISAPI/AccessControl/UserInfo/Record?format=json  (JSON person record)
//!   Step 2 — POST /ISAPI/Intelligent/FDLib/FaceDataRecord?format=json (multipart: JSON + JPEG)
//!
//! D-15 LOCKED: face delete via PUT /ISAPI/AccessControl/UserInfoDetail/Delete?format=json
//!
//! Pitfall 3 (RESEARCH): Hikvision firmware truncates UTF-8 `name` field at a byte
//! boundary, not a character boundary, causing rejection if the field straddles a
//! multibyte codepoint. `truncate_utf8` avoids this.

/// Truncate a UTF-8 string to at most `max_bytes` bytes, never splitting a
/// multibyte codepoint. Returns a `&str` slice of `s`.
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Walk backwards from max_bytes until we land on a valid char boundary.
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Build the JSON body for Step 1 of the 2-step face profile push.
///
/// POST /ISAPI/AccessControl/UserInfo/Record?format=json
///
/// `face_id` is the Cronometrix-generated UUID v4 used as the Hikvision employeeNo.
/// `full_name` is truncated to 32 bytes to respect firmware limits (Pitfall 3).
pub fn build_user_info_record_body(face_id: &str, full_name: &str) -> String {
    let name = truncate_utf8(full_name, 32);
    serde_json::json!({
        "UserInfo": {
            "employeeNo": face_id,
            "name": name,
            "userType": "normal",
            "Valid": {
                "enable": true,
                "beginTime": "2000-01-01T00:00:00",
                "endTime": "2037-12-31T23:59:59"
            },
            "doorRight": "1",
            "RightPlan": [{"doorNo": 1, "planTemplateNo": "1"}]
        }
    })
    .to_string()
}

/// Build the JSON metadata part for Step 2 of the 2-step face profile push.
///
/// The multipart form has two parts:
///   - "FaceDataRecord" (application/json): this JSON string
///   - "FaceImage"      (image/jpeg):       the raw JPEG bytes
pub fn build_facedata_metadata(face_id: &str) -> String {
    serde_json::json!({
        "faceLibType": "blackFD",
        "FDID": "1",
        "FPID": face_id
    })
    .to_string()
}

/// Build the JSON body for the face delete request (D-15 LOCKED).
///
/// PUT /ISAPI/AccessControl/UserInfoDetail/Delete?format=json
pub fn build_user_delete_body(face_id: &str) -> String {
    serde_json::json!({
        "UserInfoDetail": {
            "mode": "byEmployeeNo",
            "EmployeeNoList": [{"employeeNo": face_id}]
        }
    })
    .to_string()
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_utf8_ascii_under_limit() {
        assert_eq!(truncate_utf8("hello", 10), "hello");
    }

    #[test]
    fn truncate_utf8_multibyte_boundary() {
        // "Núñez García" has multibyte codepoints; truncating at 8 bytes
        // must not split a codepoint and must return valid UTF-8.
        let result = truncate_utf8("Núñez García", 8);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        assert!(result.len() <= 8);
    }

    #[test]
    fn truncate_utf8_exact_boundary() {
        let s = "abc";
        assert_eq!(truncate_utf8(s, 3), "abc");
    }

    #[test]
    fn build_user_info_record_body_has_employee_no() {
        let face_id = "test-face-uuid-1234";
        let body = build_user_info_record_body(face_id, "Juan Pérez");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
        assert_eq!(
            parsed["UserInfo"]["employeeNo"].as_str().unwrap(),
            face_id
        );
        // name field should be present
        assert!(parsed["UserInfo"]["name"].as_str().is_some());
    }

    #[test]
    fn build_user_info_record_body_truncates_long_name() {
        let long_name = "A".repeat(100);
        let body = build_user_info_record_body("fid", &long_name);
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
        let name = parsed["UserInfo"]["name"].as_str().unwrap();
        assert!(name.len() <= 32, "name must be ≤32 bytes, got {}", name.len());
    }

    #[test]
    fn build_facedata_metadata_has_fdid_1() {
        let meta = build_facedata_metadata("my-face-id");
        let parsed: serde_json::Value = serde_json::from_str(&meta).expect("valid JSON");
        assert_eq!(parsed["FDID"].as_str().unwrap(), "1");
        assert_eq!(parsed["FPID"].as_str().unwrap(), "my-face-id");
        assert_eq!(parsed["faceLibType"].as_str().unwrap(), "blackFD");
    }

    #[test]
    fn build_user_delete_body_has_employee_no() {
        let body = build_user_delete_body("del-face-id");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
        let list = &parsed["UserInfoDetail"]["EmployeeNoList"];
        assert_eq!(list[0]["employeeNo"].as_str().unwrap(), "del-face-id");
    }
}
