use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use validator::Validate;

/// User role in the system. Matches the DB CHECK constraint: 'admin', 'supervisor', 'viewer'.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Supervisor,
    Viewer,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Supervisor => write!(f, "supervisor"),
            Role::Viewer => write!(f, "viewer"),
        }
    }
}

impl FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Role::Admin),
            "supervisor" => Ok(Role::Supervisor),
            "viewer" => Ok(Role::Viewer),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

/// JWT claims payload for both access and refresh tokens.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject: user UUID
    pub sub: String,
    /// User role
    pub role: Role,
    /// Expiry timestamp (epoch seconds)
    pub exp: i64,
    /// Issued-at timestamp (epoch seconds)
    pub iat: i64,
    /// Token type: "access" or "refresh"
    pub token_type: String,
}

/// Login request body.
#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 1, message = "Username is required"))]
    pub username: String,
    #[validate(length(min = 1, message = "Password is required"))]
    pub password: String,
}

/// Login success response body.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub user: UserInfo,
}

/// User info returned in login/refresh responses.
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub full_name: String,
    pub role: Role,
}
