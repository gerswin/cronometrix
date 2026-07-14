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
    match s {
        "checkIn" | "breakIn" | "overtimeIn" => "entry",
        "checkOut" | "breakOut" | "overTimeOut" => "exit",
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

    #[test]
    fn direction_mapping_check_in_is_entry() {
        assert_eq!(direction_for_attendance_status("checkIn"), "entry");
        assert_eq!(direction_for_attendance_status("checkOut"), "exit");
        assert_eq!(direction_for_attendance_status("breakIn"), "entry");
        assert_eq!(direction_for_attendance_status("breakOut"), "exit");
        assert_eq!(direction_for_attendance_status("overtimeIn"), "entry");
        assert_eq!(direction_for_attendance_status("overTimeOut"), "exit");
        // Undefined / empty / unknown → "entry" (conservative default per A1)
        assert_eq!(direction_for_attendance_status(""), "entry");
        assert_eq!(direction_for_attendance_status("undefined"), "entry");
        assert_eq!(
            direction_for_attendance_status("SomeNewFirmwareValue"),
            "entry"
        );
    }
}
