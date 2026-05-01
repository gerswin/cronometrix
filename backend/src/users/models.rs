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
    pub version: i64,
}

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
    pub role: Option<String>,
}
