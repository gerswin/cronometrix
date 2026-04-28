//! Unit tests for `devices::models` validators + Command enum + Debug-redact
//! on DeviceWithPlaintext. Targets the 50% baseline gap from Plan 03 (08-04A
//! bucket row 10).

use cronometrix_api::devices::models::{
    Command, CommandRequest, CreateDeviceRequest, DeviceWithPlaintext, UpdateDeviceRequest,
    validate_direction, validate_ip, validate_scheme, validate_status,
};
use validator::Validate;

// =============================================================================
// validate_scheme / validate_direction / validate_status / validate_ip
// =============================================================================

#[test]
fn validate_scheme_accepts_http_and_https() {
    assert!(validate_scheme("http").is_ok());
    assert!(validate_scheme("https").is_ok());
}

#[test]
fn validate_scheme_rejects_others() {
    assert!(validate_scheme("ftp").is_err());
    assert!(validate_scheme("HTTP").is_err()); // case-sensitive
    assert!(validate_scheme("").is_err());
    assert!(validate_scheme("ws").is_err());
}

#[test]
fn validate_direction_accepts_entry_exit() {
    assert!(validate_direction("entry").is_ok());
    assert!(validate_direction("exit").is_ok());
}

#[test]
fn validate_direction_rejects_others() {
    assert!(validate_direction("inout").is_err());
    assert!(validate_direction("Entry").is_err());
    assert!(validate_direction("").is_err());
}

#[test]
fn validate_status_accepts_active_inactive() {
    assert!(validate_status("active").is_ok());
    assert!(validate_status("inactive").is_ok());
}

#[test]
fn validate_status_rejects_others() {
    assert!(validate_status("ACTIVE").is_err());
    assert!(validate_status("deleted").is_err());
    assert!(validate_status("").is_err());
}

#[test]
fn validate_ip_accepts_ipv4() {
    assert!(validate_ip("192.168.1.1").is_ok());
    assert!(validate_ip("0.0.0.0").is_ok());
    assert!(validate_ip("255.255.255.255").is_ok());
}

#[test]
fn validate_ip_accepts_ipv6() {
    assert!(validate_ip("::1").is_ok());
    assert!(validate_ip("2001:db8::1").is_ok());
}

#[test]
fn validate_ip_rejects_garbage() {
    assert!(validate_ip("not-an-ip").is_err());
    assert!(validate_ip("999.999.999.999").is_err());
    assert!(validate_ip("").is_err());
    assert!(validate_ip("hostname.example.com").is_err());
}

// =============================================================================
// Command enum: as_str + from_request_str
// =============================================================================

#[test]
fn command_as_str_round_trip() {
    assert_eq!(Command::DoorOpen.as_str(), "door_open");
    assert_eq!(Command::Reboot.as_str(), "reboot");
    assert_eq!(Command::EnrollmentMode.as_str(), "enrollment_mode");
}

#[test]
fn command_from_request_str_recognised_values() {
    assert!(matches!(
        Command::from_request_str("door_open"),
        Some(Command::DoorOpen)
    ));
    assert!(matches!(
        Command::from_request_str("reboot"),
        Some(Command::Reboot)
    ));
    assert!(matches!(
        Command::from_request_str("enrollment_mode"),
        Some(Command::EnrollmentMode)
    ));
}

#[test]
fn command_from_request_str_rejects_unknown() {
    assert!(Command::from_request_str("door_close").is_none());
    assert!(Command::from_request_str("Door_Open").is_none());
    assert!(Command::from_request_str("").is_none());
}

#[test]
fn command_copy_clone_semantics() {
    let c = Command::Reboot;
    let _c2 = c; // Copy
    let c3 = c.clone();
    assert_eq!(c.as_str(), c3.as_str());
}

// =============================================================================
// validator::Validate on Create/Update/Command request
// =============================================================================

fn valid_create() -> CreateDeviceRequest {
    CreateDeviceRequest {
        name: "DevA".into(),
        ip: "192.168.1.10".into(),
        port: 80,
        scheme: "https".into(),
        username: "admin".into(),
        password: "secret".into(),
        direction: "entry".into(),
        allow_insecure_tls: false,
    }
}

#[test]
fn create_device_request_validate_happy() {
    let r = valid_create();
    r.validate().expect("valid input");
}

#[test]
fn create_device_request_blank_name_rejected() {
    let mut r = valid_create();
    r.name = "".into();
    let err = r.validate().expect_err("blank name");
    assert!(err.to_string().contains("name"));
}

#[test]
fn create_device_request_oversize_name_rejected() {
    let mut r = valid_create();
    r.name = "a".repeat(101);
    assert!(r.validate().is_err(), "name > 100 chars");
}

#[test]
fn create_device_request_port_out_of_range() {
    let mut r = valid_create();
    r.port = 0; // below min 1
    assert!(r.validate().is_err());
    let mut r = valid_create();
    r.port = 65_536; // above max 65535
    assert!(r.validate().is_err());
    let mut r = valid_create();
    r.port = -10;
    assert!(r.validate().is_err());
}

#[test]
fn create_device_request_oversize_password_rejected() {
    let mut r = valid_create();
    r.password = "a".repeat(201);
    assert!(r.validate().is_err());
}

#[test]
fn update_device_request_optional_fields_valid_when_none() {
    let r = UpdateDeviceRequest {
        name: None,
        ip: None,
        port: None,
        scheme: None,
        username: None,
        password: None,
        direction: None,
        allow_insecure_tls: None,
        status: None,
        version: 1,
    };
    r.validate().expect("all-None update is valid");
}

#[test]
fn update_device_request_some_oversize_password_rejected() {
    let r = UpdateDeviceRequest {
        name: None,
        ip: None,
        port: None,
        scheme: None,
        username: None,
        password: Some("a".repeat(201)),
        direction: None,
        allow_insecure_tls: None,
        status: None,
        version: 1,
    };
    assert!(r.validate().is_err());
}

#[test]
fn update_device_request_some_zero_port_rejected() {
    let r = UpdateDeviceRequest {
        name: None,
        ip: None,
        port: Some(0),
        scheme: None,
        username: None,
        password: None,
        direction: None,
        allow_insecure_tls: None,
        status: None,
        version: 1,
    };
    assert!(r.validate().is_err());
}

#[test]
fn command_request_validate_happy() {
    CommandRequest {
        command: "door_open".into(),
    }
    .validate()
    .unwrap();
}

#[test]
fn command_request_blank_rejected() {
    let r = CommandRequest {
        command: "".into(),
    };
    assert!(r.validate().is_err());
}

#[test]
fn command_request_oversize_rejected() {
    let r = CommandRequest {
        command: "a".repeat(51),
    };
    assert!(r.validate().is_err());
}

// =============================================================================
// DeviceWithPlaintext manual Debug-redact (Security Domain rule)
// =============================================================================

#[test]
fn device_with_plaintext_debug_redacts_password() {
    let d = DeviceWithPlaintext {
        id: "id-1".into(),
        name: "Dev1".into(),
        base_url: "https://10.0.0.1".into(),
        username: "admin".into(),
        password: "supersecret".into(),
        direction: "entry".into(),
        allow_insecure_tls: false,
        status: "active".into(),
        version: 1,
    };
    let dbg = format!("{:?}", d);
    assert!(
        !dbg.contains("supersecret"),
        "password MUST not appear in Debug, got: {dbg}"
    );
    assert!(dbg.contains("[redacted]"), "Debug must mark redaction: {dbg}");
    // Non-sensitive fields appear.
    assert!(dbg.contains("admin"));
    assert!(dbg.contains("10.0.0.1"));
}
