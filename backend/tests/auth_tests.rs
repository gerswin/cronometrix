mod common;

#[tokio::test]
#[ignore = "Requires auth module from Plan 01-02"]
async fn auth_login_returns_jwt() {
    // Will test: POST /api/v1/auth/login with valid credentials returns access_token
    todo!("Implement after Plan 01-02 delivers auth handlers");
}

#[tokio::test]
#[ignore = "Requires auth module from Plan 01-02"]
async fn password_hashing_uses_argon2id() {
    // Will test: hash_password produces argon2id hash, verify_password validates it
    todo!("Implement after Plan 01-02 delivers auth::service");
}

#[tokio::test]
#[ignore = "Requires auth module from Plan 01-02"]
async fn rbac_middleware_blocks_unauthorized() {
    // Will test: Viewer token cannot access admin-only routes (403)
    todo!("Implement after Plan 01-02 delivers auth::rbac");
}

#[tokio::test]
#[ignore = "Requires auth module from Plan 01-02"]
async fn jwt_refresh_rotates_tokens() {
    // Will test: POST /api/v1/auth/refresh with valid cookie returns new tokens
    todo!("Implement after Plan 01-02 delivers refresh handler");
}

#[tokio::test]
#[ignore = "Requires setup module from Plan 01-02"]
async fn setup_wizard_creates_admin() {
    // Will test: POST /api/v1/setup/init creates admin, second call returns 409
    todo!("Implement after Plan 01-02 delivers setup handlers");
}
