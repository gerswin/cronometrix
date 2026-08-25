//! ISAPI EventNotificationAlert structs + helpers (Plan 02-03 Task 1).
//!
//! Hikvision `/ISAPI/Event/notification/alertStream` emits XML blocks with a
//! default namespace (`xmlns="http://www.hikvision.com/ver20/XMLSchema"` for
//! modern firmware, `ver10/XMLSchema` on older units). quick-xml's serde
//! derive does not resolve namespace-qualified element names — RESEARCH
//! "Pitfall 5" — so we strip the xmlns attribute before parsing.
//!
//! Heartbeat detection (A3): the device emits either an explicit
//! `<eventType>Heartbeat</eventType>` OR, on some firmware revisions, a
//! `videoloss` type with `eventState=inactive`. Both mean "device is alive,
//! nothing to persist".

use serde::Deserialize;

/// Root element of the multipart XML payload.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct EventNotificationAlert {
    #[serde(rename = "ipAddress", default)]
    pub ip_address: String,
    #[serde(rename = "dateTime", default)]
    pub date_time: String,
    #[serde(rename = "eventType", default)]
    pub event_type: String,
    #[serde(rename = "eventState", default)]
    pub event_state: String,
    #[serde(rename = "eventDescription", default)]
    pub event_description: String,
    #[serde(rename = "AccessControllerEvent", default)]
    pub access_controller_event: Option<AccessControllerEvent>,
}

/// Inner access-control payload carrying the attendance-relevant fields.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct AccessControllerEvent {
    #[serde(rename = "deviceName", default)]
    pub device_name: String,
    #[serde(rename = "majorEventType", default)]
    pub major_event_type: Option<i64>,
    #[serde(rename = "subEventType", default)]
    pub sub_event_type: Option<i64>,
    #[serde(rename = "employeeNoString", default)]
    pub employee_no_string: String,
    #[serde(rename = "name", default)]
    pub name: String,
    #[serde(rename = "currentVerifyMode", default)]
    pub current_verify_mode: String,
    #[serde(rename = "attendanceStatus", default)]
    pub attendance_status: String,
    #[serde(rename = "faceID", default)]
    pub face_id: String,
    #[serde(rename = "pictureURL", default)]
    pub picture_url: String,
}

impl AccessControllerEvent {
    /// The direction the device explicitly reported, or `None` when it did not.
    ///
    /// DS-K1T341CMFW V3.3.8 sends the literal string `"undefined"` when no
    /// check-in/check-out mode is configured on the reader. Treating that as a
    /// real value routed every marking through
    /// [`direction_for_attendance_status`], whose catch-all arm returns `entry`
    /// — silently overriding the per-device default the operator configured.
    pub fn reported_direction(&self) -> Option<&'static str> {
        match self.attendance_status.as_str() {
            "" | "undefined" | "invalid" => None,
            other => Some(direction_for_attendance_status(other)),
        }
    }

    /// Whether this event names somebody.
    ///
    /// Access controllers interleave door/tamper/status events (door open,
    /// door closed, case tamper) with the identity events that matter for
    /// attendance. Only the latter carry `employeeNoString` or `faceID`; the
    /// rest outnumbered them 2:1 in a hardware capture, and persisting them
    /// would fabricate an unknown-face marking every time the door moved.
    ///
    /// Note this does NOT filter unrecognised people: a face the device cannot
    /// place still reports an identity field, which then fails to resolve
    /// against `device_face_mappings` and is persisted as `is_unknown`.
    pub fn has_identity(&self) -> bool {
        !self.employee_no_string.is_empty() || !self.face_id.is_empty()
    }
}

/// Deserialize one alertStream part, accepting either wire format.
///
/// Firmware is not consistent about this: the XML `EventNotificationAlert` this
/// module was built around is what older units emit, while DS-K1T341CMFW V3.3.8
/// sends JSON with identical field names. Both map onto the same struct — the
/// serde renames already match the JSON keys — so the only thing that varies is
/// which deserializer runs.
///
/// Detection prefers the declared content type and falls back to sniffing the
/// first non-whitespace byte, because some firmware mislabels the part.
pub fn parse_alert(bytes: &[u8], content_type: &str) -> anyhow::Result<EventNotificationAlert> {
    let looks_json = content_type.contains("json")
        || bytes
            .iter()
            .find(|b| !b.is_ascii_whitespace())
            .is_some_and(|b| *b == b'{');

    if looks_json {
        return serde_json::from_slice(bytes)
            .map_err(|error| anyhow::anyhow!("JSON alert parse failed: {error}"));
    }

    let text = std::str::from_utf8(bytes)
        .map_err(|error| anyhow::anyhow!("XML alert is not valid UTF-8: {error}"))?;
    quick_xml::de::from_str(&strip_xmlns(text))
        .map_err(|error| anyhow::anyhow!("XML alert parse failed: {error}"))
}

impl EventNotificationAlert {
    /// Heartbeat detection per Assumption A3 in 02-RESEARCH.md:
    /// - `eventType=videoloss` + `eventState=inactive` (A3a — common on firmware <3.5)
    /// - `eventType=Heartbeat` (A3b — explicit heartbeat on newer firmware)
    pub fn is_heartbeat(&self) -> bool {
        (self.event_type.eq_ignore_ascii_case("videoloss")
            && self.event_state.eq_ignore_ascii_case("inactive"))
            || self.event_type.eq_ignore_ascii_case("heartbeat")
    }

    /// Parse `<dateTime>` as RFC 3339 and return UTC epoch seconds.
    /// Returns `None` if the field is absent or malformed — callers fall back
    /// to `chrono::Utc::now().timestamp()` to avoid dropping otherwise-valid
    /// events.
    pub fn captured_at_epoch(&self) -> Option<i64> {
        chrono::DateTime::parse_from_rfc3339(&self.date_time)
            .ok()
            .map(|dt| dt.timestamp())
    }
}

/// Map Hikvision `attendanceStatus` to our canonical `entry`/`exit` direction.
///
/// Assumption A1 in 02-RESEARCH — the device may emit values outside the
/// documented enum; we default unrecognised values to "entry" so the event is
/// never silently dropped. Phase 3 calculation treats spurious "entry" rows as
/// noise; a dropped event is unrecoverable.
pub fn direction_for_attendance_status(s: &str) -> &'static str {
    // Matched case-insensitively because the documented spelling and the wire
    // spelling disagree: DS-K1T341CMFW reports `overtimeOut`, while this arm was
    // originally written `overTimeOut`. The mismatch fell through to the
    // catch-all and filed every overtime departure as an ARRIVAL — silent data
    // corruption, not a dropped event. Confirmed against the device's own
    // `keyCfg/6/attendance` readback.
    let lowered = s.to_ascii_lowercase();
    match lowered.as_str() {
        "checkin" | "breakin" | "overtimein" => "entry",
        "checkout" | "breakout" | "overtimeout" => "exit",
        _ => "entry",
    }
}

/// Strip any `xmlns="..."` attribute from an XML string (Pitfall 5).
///
/// Handles both `ver10` and `ver20` schema URLs plus any future variants —
/// the attribute name `xmlns` is what we match on. We do NOT use a regex:
/// the attribute format is simple enough that a linear scan is cheaper and
/// has no dependency cost. Leaves a single trailing space if one existed
/// before the attribute (harmless for XML parsers).
pub fn strip_xmlns(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    const NEEDLE: &str = "xmlns=\"";
    while let Some(i) = rest.find(NEEDLE) {
        out.push_str(&rest[..i]);
        let after = &rest[i + NEEDLE.len()..];
        if let Some(j) = after.find('"') {
            rest = &after[j + 1..];
            // Strip a single leading space left behind, e.g. `<tag xmlns="..." version="2.0">`
            // becomes `<tag version="2.0">` rather than `<tag  version="2.0">`.
            if let Some(stripped) = rest.strip_prefix(' ') {
                rest = stripped;
            }
        } else {
            // Malformed: no closing quote — bail out preserving the remainder.
            break;
        }
    }
    out.push_str(rest);
    out
}

// =============================================================================
// Unit tests
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const K1T341_XML: &str = r#"<EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
  <ipAddress>192.168.1.10</ipAddress>
  <portNo>80</portNo>
  <protocol>HTTP</protocol>
  <macAddress>aa:bb:cc:dd:ee:ff</macAddress>
  <channelID>1</channelID>
  <dateTime>2024-04-19T12:34:56+00:00</dateTime>
  <activePostCount>1</activePostCount>
  <eventType>AccessControllerEvent</eventType>
  <eventState>active</eventState>
  <eventDescription>Access Controller Event</eventDescription>
  <AccessControllerEvent>
    <deviceName>DS-K1T341</deviceName>
    <majorEventType>5</majorEventType>
    <subEventType>75</subEventType>
    <employeeNoString>EMP001</employeeNoString>
    <name>John Doe</name>
    <cardNo>0</cardNo>
    <cardType>1</cardType>
    <currentVerifyMode>face</currentVerifyMode>
    <attendanceStatus>checkIn</attendanceStatus>
    <faceID>42</faceID>
    <pictureURL>/ISAPI/Intelligent/FDLib/pictureUpload?id=42</pictureURL>
  </AccessControllerEvent>
</EventNotificationAlert>"#;

    #[test]
    fn deserialize_k1t341_fixture() {
        let stripped = strip_xmlns(K1T341_XML);
        let alert: EventNotificationAlert =
            quick_xml::de::from_str(&stripped).expect("should parse k1t341 XML");
        assert_eq!(alert.event_type, "AccessControllerEvent");
        let ace = alert
            .access_controller_event
            .as_ref()
            .expect("has AccessControllerEvent");
        assert_eq!(ace.employee_no_string, "EMP001");
        assert_eq!(ace.face_id, "42");
        assert_eq!(ace.attendance_status, "checkIn");
        // 2024-04-19T12:34:56+00:00 -> epoch 1713530096
        assert_eq!(alert.captured_at_epoch(), Some(1713530096));
    }

    #[test]
    fn strip_xmlns_removes_ver20() {
        let input = r#"<EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema"><a/></EventNotificationAlert>"#;
        let out = strip_xmlns(input);
        assert!(!out.contains("xmlns"));
        // Still parseable
        assert!(out.contains("<EventNotificationAlert"));
    }

    #[test]
    fn strip_xmlns_removes_ver10() {
        let input = r#"<EventNotificationAlert xmlns="http://www.hikvision.com/ver10/XMLSchema"><a/></EventNotificationAlert>"#;
        let out = strip_xmlns(input);
        assert!(!out.contains("xmlns"));
    }

    #[test]
    fn is_heartbeat_detects_videoloss_inactive() {
        let alert = EventNotificationAlert {
            event_type: "videoloss".to_string(),
            event_state: "inactive".to_string(),
            ..EventNotificationAlert::default()
        };
        assert!(alert.is_heartbeat());
    }

    #[test]
    fn is_heartbeat_detects_explicit_heartbeat() {
        let alert = EventNotificationAlert {
            event_type: "Heartbeat".to_string(),
            ..EventNotificationAlert::default()
        };
        assert!(alert.is_heartbeat());
        // Case-insensitive
        let alert2 = EventNotificationAlert {
            event_type: "heartbeat".to_string(),
            ..EventNotificationAlert::default()
        };
        assert!(alert2.is_heartbeat());
    }

    #[test]
    fn is_heartbeat_false_for_access_event() {
        let alert = EventNotificationAlert {
            event_type: "AccessControllerEvent".to_string(),
            event_state: "active".to_string(),
            ..EventNotificationAlert::default()
        };
        assert!(!alert.is_heartbeat());
    }

    #[test]
    fn is_heartbeat_false_for_videoloss_active() {
        // videoloss+ACTIVE would be a genuine camera-feed loss event, not a heartbeat.
        let alert = EventNotificationAlert {
            event_type: "videoloss".to_string(),
            event_state: "active".to_string(),
            ..EventNotificationAlert::default()
        };
        assert!(!alert.is_heartbeat());
    }

    /// Verbatim event from DS-K1T341CMFW firmware V3.3.8. This firmware emits
    /// JSON, not the XML this module was originally written around, and the
    /// mismatch meant every marking was silently discarded on real hardware.
    const K1T341_JSON_RECOGNITION: &str = r#"{
        "ipAddress": "192.168.1.181",
        "portNo": 80,
        "macAddress": "e0:ca:3c:f5:02:6e",
        "channelID": 1,
        "dateTime": "2026-08-02T10:15:30-04:00",
        "activePostCount": 1,
        "eventType": "AccessControllerEvent",
        "eventState": "active",
        "eventDescription": "Access Controller Event",
        "AccessControllerEvent": {
            "deviceName": "Access Controller",
            "majorEventType": 5,
            "subEventType": 75,
            "name": "GIAN CARDENAS",
            "employeeNoString": "15957875",
            "currentVerifyMode": "cardOrFaceOrFp",
            "attendanceStatus": "undefined",
            "serialNo": 30203
        }
    }"#;

    /// Door movement: same envelope, nobody named.
    const K1T341_JSON_DOOR: &str = r#"{
        "dateTime": "2026-08-02T10:15:29-04:00",
        "eventType": "AccessControllerEvent",
        "AccessControllerEvent": {
            "majorEventType": 5,
            "subEventType": 22,
            "doorNo": 1,
            "attendanceStatus": "undefined"
        }
    }"#;

    #[test]
    fn parse_alert_reads_the_json_wire_format() {
        let alert = parse_alert(
            K1T341_JSON_RECOGNITION.as_bytes(),
            "application/json; charset=\"UTF-8\"",
        )
        .expect("JSON alert must parse");
        let ace = alert
            .access_controller_event
            .as_ref()
            .expect("AccessControllerEvent present");
        assert_eq!(ace.employee_no_string, "15957875");
        assert_eq!(ace.name, "GIAN CARDENAS");
        assert_eq!(ace.sub_event_type, Some(75));
        assert!(ace.has_identity());
        assert_eq!(
            alert.captured_at_epoch(),
            Some(
                chrono::DateTime::parse_from_rfc3339("2026-08-02T10:15:30-04:00")
                    .unwrap()
                    .timestamp()
            )
        );
    }

    /// Firmware has been seen mislabelling parts, so the body wins when the
    /// declared content type disagrees with what was actually sent.
    #[test]
    fn parse_alert_sniffs_json_when_content_type_lies() {
        let alert = parse_alert(K1T341_JSON_RECOGNITION.as_bytes(), "application/xml")
            .expect("body sniffing must win over a wrong content type");
        assert_eq!(
            alert
                .access_controller_event
                .expect("event")
                .employee_no_string,
            "15957875"
        );
    }

    #[test]
    fn parse_alert_still_reads_the_xml_wire_format() {
        let alert =
            parse_alert(K1T341_XML.as_bytes(), "application/xml").expect("XML alert must parse");
        assert!(alert.access_controller_event.is_some());
    }

    /// `undefined` is what the reader sends when no check-in/check-out mode is
    /// configured. Treating it as a real value routed every marking to `entry`,
    /// overriding the per-device default the operator chose.
    #[test]
    fn undefined_attendance_status_reports_no_direction() {
        let alert = parse_alert(K1T341_JSON_RECOGNITION.as_bytes(), "application/json").unwrap();
        assert_eq!(
            alert.access_controller_event.unwrap().reported_direction(),
            None
        );
    }

    #[test]
    fn explicit_attendance_status_still_reports_a_direction() {
        let mut ace = AccessControllerEvent {
            attendance_status: "checkOut".into(),
            ..Default::default()
        };
        assert_eq!(ace.reported_direction(), Some("exit"));
        ace.attendance_status = "checkIn".into();
        assert_eq!(ace.reported_direction(), Some("entry"));
    }

    #[test]
    fn door_events_carry_no_identity() {
        let alert = parse_alert(K1T341_JSON_DOOR.as_bytes(), "application/json").unwrap();
        assert!(!alert.access_controller_event.unwrap().has_identity());
    }

    #[test]
    fn face_id_alone_counts_as_an_identity() {
        let ace = AccessControllerEvent {
            face_id: "d4c6112e831e4070a3ac0fbf88575709".into(),
            ..Default::default()
        };
        assert!(ace.has_identity());
    }

    #[test]
    fn direction_mapping_check_in_is_entry() {
        assert_eq!(direction_for_attendance_status("checkIn"), "entry");
        assert_eq!(direction_for_attendance_status("checkOut"), "exit");
        assert_eq!(direction_for_attendance_status("breakIn"), "entry");
        assert_eq!(direction_for_attendance_status("breakOut"), "exit");
        assert_eq!(direction_for_attendance_status("overtimeIn"), "entry");
        assert_eq!(direction_for_attendance_status("overTimeOut"), "exit");
        // The spelling the hardware actually sends — this is the one that was
        // silently mapped to "entry" before.
        assert_eq!(direction_for_attendance_status("overtimeOut"), "exit");
    }

    /// Every status the device can emit, taken from its own `keyCfg/N/attendance`
    /// readback after the six function keys were configured. Casing must not
    /// decide whether a departure is recorded as an arrival.
    #[test]
    fn every_hardware_attendance_status_maps_to_the_right_direction() {
        for (status, expected) in [
            ("checkIn", "entry"),
            ("checkOut", "exit"),
            ("breakOut", "exit"),
            ("breakIn", "entry"),
            ("overtimeIn", "entry"),
            ("overtimeOut", "exit"),
        ] {
            assert_eq!(
                direction_for_attendance_status(status),
                expected,
                "{status} must map to {expected}"
            );
        }
        // Undefined / empty / unknown → "entry" (conservative default per A1)
        assert_eq!(direction_for_attendance_status(""), "entry");
        assert_eq!(direction_for_attendance_status("undefined"), "entry");
        assert_eq!(
            direction_for_attendance_status("SomeNewFirmwareValue"),
            "entry"
        );
    }
}
