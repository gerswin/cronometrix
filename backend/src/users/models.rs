use serde::{Deserialize, Serialize};
use validator::Validate;

/// User record returned by GET endpoints. Excludes password_hash.
#[derive(Debug, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub full_name: String,
    pub role: String, // "admin" | "supervisor" | "viewer"
    pub status: String,
    pub deleted_at: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 1, max = 100, message = "Username is required (1-100 chars)"))]
    pub username: String,
    #[validate(length(min = 1, max = 200, message = "Full name is required (1-200 chars)"))]
    pub full_name: String,
    /// "admin" | "supervisor" | "viewer"
    pub role: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
    /// Optional department scope (H-11). `None` = org-wide (no scope).
    pub department_id: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 200))]
    pub full_name: Option<String>,
    pub role: Option<String>,
    /// Optional password reset; min 8 chars when present.
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: Option<String>,
    /// Optional status change ("active" | "inactive").
    pub status: Option<String>,
    /// Tri-state department scope (H-11): field omitted = `None` (leave as-is);
    /// explicit JSON `null` = `Some(None)` (clear to org-wide); a value =
    /// `Some(Some(id))` (assign). Needs `double_option` because a plain
    /// `Option<String>` cannot tell an omitted field from an explicit null.
    #[serde(default, deserialize_with = "double_option")]
    pub department_id: Option<Option<String>>,
    pub version: i64,
}

/// serde helper for a tri-state PATCH field: distinguishes an omitted field
/// (`None`), an explicit JSON `null` (`Some(None)`), and a value
/// (`Some(Some(v))`).
fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Deserialize::deserialize(de).map(Some)
}

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
    pub role: Option<String>,
}
