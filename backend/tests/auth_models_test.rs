//! Unit tests for `auth::models` — Role enum (Display + FromStr), Claims
//! Serialize/Deserialize roundtrip, LoginRequest validation. Targets the
//! 30.77% baseline gap from Plan 03 (08-04A bucket row 3).

use cronometrix_api::auth::models::{Claims, LoginRequest, LoginResponse, Role, UserInfo};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::str::FromStr;
use validator::Validate;

fn decode_token_claims(token: &str) -> serde_json::Value {
    decode::<serde_json::Value>(
        token,
        &DecodingKey::from_secret(b"test-secret-key-at-least-32-characters-long!!"),
        &Validation::default(),
    )
    .expect("issued token must decode")
    .claims
}

#[test]
fn role_display_admin_supervisor_viewer() {
    assert_eq!(Role::Admin.to_string(), "admin");
    assert_eq!(Role::Supervisor.to_string(), "supervisor");
    assert_eq!(Role::Viewer.to_string(), "viewer");
}

#[test]
fn role_from_str_recognised() {
    assert_eq!(Role::from_str("admin").unwrap(), Role::Admin);
    assert_eq!(Role::from_str("supervisor").unwrap(), Role::Supervisor);
    assert_eq!(Role::from_str("viewer").unwrap(), Role::Viewer);
}

#[test]
fn role_from_str_rejects_unknown() {
    let err = Role::from_str("root").unwrap_err();
    assert!(err.contains("Unknown role"), "err = {err}");
    assert!(err.contains("root"), "err must echo bad value, got: {err}");
}

#[test]
fn role_from_str_rejects_empty() {
    let err = Role::from_str("").unwrap_err();
    assert!(err.contains("Unknown role"), "empty string rejected: {err}");
}

#[test]
fn role_from_str_case_sensitive() {
    // Documented behaviour: matcher is exact-lowercase. Capitalised forms reject.
    assert!(Role::from_str("Admin").is_err());
    assert!(Role::from_str("ADMIN").is_err());
}

#[test]
fn role_partial_eq_and_clone() {
    let r = Role::Supervisor;
    let r2 = r.clone();
    assert_eq!(r, r2);
    assert_ne!(Role::Admin, Role::Supervisor);
}

#[test]
fn role_serialize_roundtrip_via_serde_json() {
    let s = serde_json::to_string(&Role::Admin).unwrap();
    assert_eq!(s, "\"admin\"");
    let back: Role = serde_json::from_str("\"admin\"").unwrap();
    assert_eq!(back, Role::Admin);

    let s = serde_json::to_string(&Role::Supervisor).unwrap();
    assert_eq!(s, "\"supervisor\"");

    let s = serde_json::to_string(&Role::Viewer).unwrap();
    assert_eq!(s, "\"viewer\"");
}

#[test]
fn role_deserialize_rejects_unknown_string() {
    let result: Result<Role, _> = serde_json::from_str("\"banana\"");
    assert!(result.is_err());
}

#[test]
fn claims_serialize_deserialize_roundtrip() {
    let c = Claims {
        sub: "user-1".into(),
        role: Role::Admin,
        exp: 1_700_000_000,
        iat: 1_700_000_000 - 1200,
        jti: uuid::Uuid::new_v4().to_string(),
        token_type: "access".into(),
    };
    let s = serde_json::to_string(&c).unwrap();
    let back: Claims = serde_json::from_str(&s).unwrap();
    assert_eq!(back.sub, c.sub);
    assert_eq!(back.role, c.role);
    assert_eq!(back.exp, c.exp);
    assert_eq!(back.iat, c.iat);
    assert_eq!(back.jti, c.jti);
    assert_eq!(back.token_type, c.token_type);
}

#[test]
fn claims_clone_preserves_fields() {
    let c = Claims {
        sub: "u".into(),
        role: Role::Viewer,
        exp: 0,
        iat: 0,
        jti: uuid::Uuid::new_v4().to_string(),
        token_type: "refresh".into(),
    };
    let c2 = c.clone();
    assert_eq!(c.sub, c2.sub);
    assert_eq!(c.role, c2.role);
    assert_eq!(c.jti, c2.jti);
}

#[test]
fn claims_deserialization_rejects_missing_jti() {
    let claims_without_jti = serde_json::json!({
        "sub": "user-1",
        "role": "admin",
        "exp": 1_900_000_000,
        "iat": 1_800_000_000,
        "token_type": "access"
    });

    let result = serde_json::from_value::<Claims>(claims_without_jti);
    assert!(result.is_err(), "jti must be a required claim");
}

#[test]
fn token_issuers_emit_non_empty_uuid_jtis() {
    let secret = b"test-secret-key-at-least-32-characters-long!!";
    let access =
        cronometrix_api::auth::service::issue_access_token("user-1", &Role::Admin, secret).unwrap();
    let refresh =
        cronometrix_api::auth::service::issue_refresh_token("user-1", &Role::Admin, secret)
            .unwrap();

    for token in [&access, &refresh] {
        let claims = decode_token_claims(token);
        let jti = claims["jti"]
            .as_str()
            .expect("every issued token must carry a string jti");
        assert!(!jti.is_empty(), "issued jti must not be empty");
        uuid::Uuid::parse_str(jti).expect("issued jti must be a UUID");
    }
}

#[test]
fn back_to_back_access_and_refresh_tokens_have_distinct_jtis() {
    let secret = b"test-secret-key-at-least-32-characters-long!!";
    let tokens = [
        cronometrix_api::auth::service::issue_access_token("user-1", &Role::Admin, secret).unwrap(),
        cronometrix_api::auth::service::issue_access_token("user-1", &Role::Admin, secret).unwrap(),
        cronometrix_api::auth::service::issue_refresh_token("user-1", &Role::Admin, secret)
            .unwrap(),
        cronometrix_api::auth::service::issue_refresh_token("user-1", &Role::Admin, secret)
            .unwrap(),
    ];
    let jtis: std::collections::HashSet<String> = tokens
        .iter()
        .map(|token| {
            decode_token_claims(token)["jti"]
                .as_str()
                .expect("every issued token must carry a jti")
                .to_string()
        })
        .collect();

    assert_eq!(jtis.len(), tokens.len(), "every issued jti must be unique");
}

#[test]
fn login_request_validate_accepts_non_empty() {
    let body = LoginRequest {
        username: "u".into(),
        password: "p".into(),
    };
    body.validate().expect("non-empty creds pass validation");
}

#[test]
fn login_request_validate_rejects_empty_username() {
    let body = LoginRequest {
        username: String::new(),
        password: "p".into(),
    };
    let err = body.validate().expect_err("empty username must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("username"),
        "validator must mention field: {msg}"
    );
}

#[test]
fn login_request_validate_rejects_empty_password() {
    let body = LoginRequest {
        username: "u".into(),
        password: String::new(),
    };
    let err = body.validate().expect_err("empty password must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("password"),
        "validator must mention field: {msg}"
    );
}

#[test]
fn login_request_validate_rejects_both_empty() {
    let body = LoginRequest {
        username: String::new(),
        password: String::new(),
    };
    body.validate().expect_err("both empty must fail");
}

#[test]
fn login_request_deserializes_from_json() {
    let raw = "{\"username\":\"alice\",\"password\":\"secret\"}";
    let body: LoginRequest = serde_json::from_str(raw).unwrap();
    assert_eq!(body.username, "alice");
    assert_eq!(body.password, "secret");
}

#[test]
fn login_response_serializes_with_user_info() {
    let resp = LoginResponse {
        access_token: "tok".into(),
        user: UserInfo {
            id: "id-1".into(),
            username: "alice".into(),
            full_name: "Alice".into(),
            role: Role::Admin,
        },
    };
    let v: serde_json::Value = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["access_token"], "tok");
    assert_eq!(v["user"]["username"], "alice");
    assert_eq!(v["user"]["role"], "admin"); // lowercase via Role serde
}
