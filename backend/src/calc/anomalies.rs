use serde::{Deserialize, Serialize};

/// Engine-emitted anomaly codes per D-18. The SCREAMING_SNAKE_CASE string form
/// returned by [`AnomalyCode::as_str`] matches the CHECK constraint in
/// `008_daily_record_anomalies.sql` byte-for-byte — keep them in sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnomalyCode {
    MissingEntry,
    MissingExit,
    UnknownFaceInWindow,
    LunchPunchMissing,
    OtCapExceededDaily,
    OtCapExceededWeekly,
    OtCapExceededAnnual,
    EventsOnLeaveDay,
    RecomputeAfterEdit,
    OvernightInferenceAmbiguous,
}

impl AnomalyCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingEntry => "MISSING_ENTRY",
            Self::MissingExit => "MISSING_EXIT",
            Self::UnknownFaceInWindow => "UNKNOWN_FACE_IN_WINDOW",
            Self::LunchPunchMissing => "LUNCH_PUNCH_MISSING",
            Self::OtCapExceededDaily => "OT_CAP_EXCEEDED_DAILY",
            Self::OtCapExceededWeekly => "OT_CAP_EXCEEDED_WEEKLY",
            Self::OtCapExceededAnnual => "OT_CAP_EXCEEDED_ANNUAL",
            Self::EventsOnLeaveDay => "EVENTS_ON_LEAVE_DAY",
            Self::RecomputeAfterEdit => "RECOMPUTE_AFTER_EDIT",
            Self::OvernightInferenceAmbiguous => "OVERNIGHT_INFERENCE_AMBIGUOUS",
        }
    }
}
