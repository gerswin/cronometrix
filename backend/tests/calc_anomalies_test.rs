//! Unit tests for `calc::anomalies::AnomalyCode`. Targets the 61.54% baseline
//! gap from Plan 03 (08-04A bucket row 4). `as_str()` is exercised for every
//! variant; serde + Hash + Copy/Clone semantics covered.

use cronometrix_api::calc::anomalies::AnomalyCode;
use std::collections::HashSet;

#[test]
fn as_str_matches_check_constraint_strings() {
    // The values below must match `008_daily_record_anomalies.sql` byte-for-byte.
    assert_eq!(AnomalyCode::MissingEntry.as_str(), "MISSING_ENTRY");
    assert_eq!(AnomalyCode::MissingExit.as_str(), "MISSING_EXIT");
    assert_eq!(
        AnomalyCode::UnknownFaceInWindow.as_str(),
        "UNKNOWN_FACE_IN_WINDOW"
    );
    assert_eq!(
        AnomalyCode::LunchPunchMissing.as_str(),
        "LUNCH_PUNCH_MISSING"
    );
    assert_eq!(
        AnomalyCode::OtCapExceededDaily.as_str(),
        "OT_CAP_EXCEEDED_DAILY"
    );
    assert_eq!(
        AnomalyCode::OtCapExceededWeekly.as_str(),
        "OT_CAP_EXCEEDED_WEEKLY"
    );
    assert_eq!(
        AnomalyCode::OtCapExceededAnnual.as_str(),
        "OT_CAP_EXCEEDED_ANNUAL"
    );
    assert_eq!(
        AnomalyCode::EventsOnLeaveDay.as_str(),
        "EVENTS_ON_LEAVE_DAY"
    );
    assert_eq!(
        AnomalyCode::RecomputeAfterEdit.as_str(),
        "RECOMPUTE_AFTER_EDIT"
    );
    assert_eq!(
        AnomalyCode::OvernightInferenceAmbiguous.as_str(),
        "OVERNIGHT_INFERENCE_AMBIGUOUS"
    );
}

#[test]
fn all_variants_distinct_in_hashset() {
    let mut set: HashSet<AnomalyCode> = HashSet::new();
    set.insert(AnomalyCode::MissingEntry);
    set.insert(AnomalyCode::MissingExit);
    set.insert(AnomalyCode::UnknownFaceInWindow);
    set.insert(AnomalyCode::LunchPunchMissing);
    set.insert(AnomalyCode::OtCapExceededDaily);
    set.insert(AnomalyCode::OtCapExceededWeekly);
    set.insert(AnomalyCode::OtCapExceededAnnual);
    set.insert(AnomalyCode::EventsOnLeaveDay);
    set.insert(AnomalyCode::RecomputeAfterEdit);
    set.insert(AnomalyCode::OvernightInferenceAmbiguous);
    assert_eq!(set.len(), 10, "10 distinct anomaly codes");
}

#[test]
fn copy_and_clone_semantics() {
    let a = AnomalyCode::OtCapExceededDaily;
    let b = a; // Copy
    let c = a.clone(); // Clone
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(b.as_str(), "OT_CAP_EXCEEDED_DAILY");
}

#[test]
fn partial_eq_distinguishes_variants() {
    assert_ne!(AnomalyCode::MissingEntry, AnomalyCode::MissingExit);
    assert_eq!(AnomalyCode::MissingEntry, AnomalyCode::MissingEntry);
}

#[test]
fn serialize_each_variant_to_json() {
    // The enum derives Serialize without a tagged repr, so each variant
    // becomes its PascalCase Rust name. as_str() is the SCREAMING_SNAKE form
    // used in the DB; the JSON form is the wire format used internally.
    assert_eq!(
        serde_json::to_string(&AnomalyCode::MissingEntry).unwrap(),
        "\"MissingEntry\""
    );
    assert_eq!(
        serde_json::to_string(&AnomalyCode::OvernightInferenceAmbiguous).unwrap(),
        "\"OvernightInferenceAmbiguous\""
    );
}

#[test]
fn deserialize_json_roundtrip() {
    let s = serde_json::to_string(&AnomalyCode::EventsOnLeaveDay).unwrap();
    let back: AnomalyCode = serde_json::from_str(&s).unwrap();
    assert_eq!(back, AnomalyCode::EventsOnLeaveDay);
}

#[test]
fn deserialize_unknown_variant_rejected() {
    let r: Result<AnomalyCode, _> = serde_json::from_str("\"NotAnAnomaly\"");
    assert!(r.is_err());
}

#[test]
fn debug_impl_renders_variant_name() {
    let s = format!("{:?}", AnomalyCode::LunchPunchMissing);
    assert!(s.contains("LunchPunchMissing"));
}
