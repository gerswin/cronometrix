//! Bloque 4 (H-11, Task 2): the access-scope authorization dimension.
//!
//! `ActorScope` is derived from the authenticated `Claims` and is the single
//! place that answers "which department is this actor confined to?". It is
//! deny-by-default: a scoped actor sees only its own department, and a resource
//! with no department (or a different one) is denied. The ONLY legitimate
//! `Unscoped` actor is an admin, or an org-wide user whose `department_id` is
//! NULL (D1). Every other path MUST apply the scope — an unfiltered query is a
//! bug, not a default.

use crate::auth::models::{Claims, Role};

/// The department confinement derived from an actor's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorScope {
    /// No department confinement: sees every department. Legitimate only for an
    /// admin, or a non-admin whose `department_id` is NULL (org-wide, D1).
    Unscoped,
    /// Confined to a single department (D3: one department per user).
    Department(String),
}

impl ActorScope {
    /// Derive the scope from JWT claims. Admin is always `Unscoped`; a
    /// supervisor/viewer is `Department` when it carries a `department_id`, and
    /// `Unscoped` (org-wide) when it does not (D1: NULL = org-wide).
    pub fn from_claims(claims: &Claims) -> Self {
        if claims.role == Role::Admin {
            return ActorScope::Unscoped;
        }
        match &claims.department_id {
            Some(dept) => ActorScope::Department(dept.clone()),
            None => ActorScope::Unscoped,
        }
    }

    /// The department this actor is confined to, or `None` when unscoped. List
    /// queries add a `department_id = ?` predicate when this is `Some`; an
    /// `Unscoped` actor adds no predicate and sees every row.
    pub fn department_id(&self) -> Option<&str> {
        match self {
            ActorScope::Unscoped => None,
            ActorScope::Department(dept) => Some(dept),
        }
    }

    /// Whether the actor sees every department (no scope predicate applies).
    pub fn is_unscoped(&self) -> bool {
        matches!(self, ActorScope::Unscoped)
    }

    /// Deny-by-default access check for a single resource, keyed by the
    /// department the resource belongs to. An `Unscoped` actor is permitted
    /// everything; a `Department` actor is permitted only its own department,
    /// and a resource with no department (`None`) is denied.
    pub fn permits(&self, resource_department: Option<&str>) -> bool {
        match self {
            ActorScope::Unscoped => true,
            ActorScope::Department(dept) => resource_department == Some(dept.as_str()),
        }
    }
}
