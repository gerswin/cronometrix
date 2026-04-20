use cronometrix_api::db::run_migrations;

/// Deterministic 32-byte key (base64) used by every test that spins up a Config
/// with device-credential crypto wired in. DO NOT use in production.
pub const TEST_DEVICE_CREDS_KEY_B64: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";

pub fn test_device_creds_key() -> [u8; 32] {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD
        .decode(TEST_DEVICE_CREDS_KEY_B64)
        .expect("test key is valid base64")
        .as_slice()
        .try_into()
        .expect("test key decodes to 32 bytes")
}

/// Create a temporary file-based libSQL database with all migrations applied.
/// Each test gets its own isolated database instance via a unique temp path.
///
/// NOTE: We use a temp file (not :memory:) because each call to db.connect() on
/// an :memory: database opens a NEW isolated SQLite connection with no shared state.
/// A temp file ensures all connections see the same schema.
pub async fn test_db() -> libsql::Database {
    // Generate a unique temp path per call so tests are isolated from each other
    let tmp_path = format!("/tmp/cronometrix_test_{}.db", uuid::Uuid::new_v4());

    let db = libsql::Builder::new_local(&tmp_path)
        .build()
        .await
        .expect("Failed to create test database");

    let conn = db.connect().expect("Failed to connect to test database");

    // Run migrations via the same production code path
    run_migrations(&conn)
        .await
        .expect("Failed to run migrations in test database");

    db
}

/// Generate a test JWT access token for a given role.
/// Uses a fixed test secret — NEVER use in production.
pub const TEST_JWT_SECRET: &str = "test-secret-key-at-least-32-characters-long!!";

pub fn test_access_token(user_id: &str, role: &str) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;

    let claims = json!({
        "sub": user_id,
        "role": role,
        "exp": chrono::Utc::now().timestamp() + 3600,  // 1 hour for tests
        "iat": chrono::Utc::now().timestamp(),
        "token_type": "access"
    });

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .expect("Failed to create test token")
}

/// Create a test admin user directly in the database.
/// Returns the user ID.
pub async fn create_test_admin(db: &libsql::Database) -> String {
    let conn = db.connect().expect("Failed to connect");
    let user_id = uuid::Uuid::new_v4().to_string();

    // Use a pre-hashed password for speed in tests
    // The actual hash is for "testpassword123"
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) VALUES (?1, 'testadmin', 'Test Admin', ?2, 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id.clone(), "$argon2id$v=19$m=19456,t=2,p=1$placeholder_test_hash"],
    )
    .await
    .expect("Failed to create test admin");

    user_id
}
