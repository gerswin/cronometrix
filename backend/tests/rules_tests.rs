mod common;

#[tokio::test]
#[ignore = "Requires rules module from Plan 01-03"]
async fn rules_tolerance_endpoint() {
    // Will test: GET /rules returns singleton, PATCH /rules updates tolerances
    todo!("Implement after Plan 01-03 delivers rules handlers");
}

#[tokio::test]
#[ignore = "Requires rules module from Plan 01-03"]
async fn rules_bonus_minutes_config() {
    // Will test: PATCH /rules with bonus_minutes updates the value
    todo!("Implement after Plan 01-03 delivers rules update");
}

#[tokio::test]
#[ignore = "Requires rules module from Plan 01-03"]
async fn rules_effective_from_updates_on_change() {
    // Will test: PATCH /rules sets effective_from to current timestamp
    todo!("Implement after Plan 01-03 delivers effective_from logic");
}
