//! Bloque 4 (H-11, Task 2): ActorScope derivation and the deny-by-default
//! access predicate. Admin (and NULL-department, D1) are the only unscoped
//! actors; a scoped actor sees only its own department, and a resource with no
//! department is denied to it.

use cronometrix_api::auth::models::{Claims, Role};
use cronometrix_api::auth::scope::ActorScope;

fn claims(role: Role, department_id: Option<&str>) -> Claims {
    Claims {
        sub: "user-1".into(),
        role,
        department_id: department_id.map(String::from),
        exp: 0,
        iat: 0,
        jti: "jti".into(),
        token_type: "access".into(),
    }
}

#[test]
fn admin_is_always_unscoped_even_with_a_department() {
    // An admin sees every department regardless of any department_id on the token.
    let scope = ActorScope::from_claims(&claims(Role::Admin, Some("dept-A")));
    assert_eq!(scope, ActorScope::Unscoped);
    assert!(scope.is_unscoped());
    assert_eq!(scope.department_id(), None);
}

#[test]
fn a_scoped_supervisor_is_confined_to_its_department() {
    let scope = ActorScope::from_claims(&claims(Role::Supervisor, Some("dept-A")));
    assert_eq!(scope, ActorScope::Department("dept-A".into()));
    assert_eq!(scope.department_id(), Some("dept-A"));
    assert!(!scope.is_unscoped());
}

#[test]
fn a_null_department_non_admin_is_org_wide() {
    // D1: a NULL department = org-wide, not denied.
    for role in [Role::Supervisor, Role::Viewer] {
        let scope = ActorScope::from_claims(&claims(role, None));
        assert_eq!(scope, ActorScope::Unscoped);
    }
}

#[test]
fn unscoped_permits_every_resource_including_departmentless_ones() {
    let scope = ActorScope::Unscoped;
    assert!(scope.permits(Some("dept-A")));
    assert!(scope.permits(Some("dept-B")));
    assert!(scope.permits(None));
}

#[test]
fn a_scoped_actor_permits_only_its_own_department() {
    let scope = ActorScope::Department("dept-A".into());
    assert!(scope.permits(Some("dept-A")));
    assert!(!scope.permits(Some("dept-B")));
    // Deny-by-default: a resource with no department is denied to a scoped actor.
    assert!(!scope.permits(None));
}
