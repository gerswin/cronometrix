//! Bloque 4 (H-11, Task 4c): a report is confined to the actor's department.
//! `scoped_department_ids` imposes the actor's scope over the request's
//! `department_ids`, so a scoped actor can never widen a report beyond its own
//! department (the payroll of every department was previously reachable by
//! omitting the filter).

use cronometrix_api::auth::scope::ActorScope;
use cronometrix_api::reports::service::scoped_department_ids;

#[test]
fn an_unscoped_actor_keeps_the_requested_department_filter() {
    let scope = ActorScope::Unscoped;
    assert_eq!(scoped_department_ids(&scope, None), None);
    assert_eq!(
        scoped_department_ids(&scope, Some(vec!["dept-x".into()])),
        Some(vec!["dept-x".to_string()])
    );
}

#[test]
fn a_scoped_actor_is_confined_to_its_department_regardless_of_request() {
    let scope = ActorScope::Department("dept-a".into());
    // Omitted filter -> only its own department.
    assert_eq!(
        scoped_department_ids(&scope, None),
        Some(vec!["dept-a".to_string()])
    );
    // Requesting another department -> still only its own (never B).
    assert_eq!(
        scoped_department_ids(&scope, Some(vec!["dept-b".into()])),
        Some(vec!["dept-a".to_string()])
    );
}
