//! The inbound port's data shape.
//!
//! Adapters translate their vendor payload into this and hand it to
//! `attendance::ingest`. Nothing here names a manufacturer, a protocol or a
//! wire format — that is what lets a second reader brand reuse the resolution
//! and persistence logic instead of reimplementing it.

/// One authentication a reader reported, stripped of vendor framing.
pub struct RawMarking {
    /// The identifier the device knows this person by. On Hikvision this is
    /// whatever was pushed as `UserInfo.employeeNo`, echoed back in
    /// `employeeNoString`; other vendors use other fields.
    pub external_person_id: Option<String>,
    /// A separate face-library identifier, when the device reports one apart
    /// from the person id. Both are tried against `device_face_mappings`.
    pub face_id: Option<String>,
    /// UTC epoch seconds. Adapters resolve their own clock format; the domain
    /// never parses a device timestamp.
    pub occurred_at: i64,
    /// `"entry"` or `"exit"` when the reader reported one. `None` means the
    /// device did not say, and the device's configured default applies.
    pub direction: Option<String>,
    pub photo: Option<Vec<u8>>,
    /// Verbatim payload, kept for forensic re-parsing (D-12).
    pub raw_payload: String,
}

impl RawMarking {
    /// Whether this names somebody.
    ///
    /// Readers interleave door, tamper and status notifications with real
    /// markings; only the latter carry an identity.
    pub fn has_identity(&self) -> bool {
        self.external_person_id
            .as_deref()
            .is_some_and(|value| !value.is_empty())
            || self
                .face_id
                .as_deref()
                .is_some_and(|value| !value.is_empty())
    }
}
