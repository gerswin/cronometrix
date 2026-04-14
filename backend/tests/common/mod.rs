/// Create an in-memory libSQL database with all migrations applied.
/// Each test gets its own isolated database instance.
///
/// NOTE: These include_str! macros reference placeholder files during Wave 0.
/// Plan 01-01 overwrites the placeholders with real SQL. After Plan 01-01,
/// the db_tests (schema, audit, epoch) will pass. Before that, only
/// compilation is guaranteed.
pub async fn test_db() -> libsql::Database {
    // Use in-memory SQLite for tests — fast, isolated, no cleanup needed
    let db = libsql::Builder::new_local(":memory:")
        .build()
        .await
        .expect("Failed to create test database");

    let conn = db.connect().expect("Failed to connect to test database");

    // Apply migrations manually (same SQL as production migrations)
    // During Wave 0 these are placeholders; Plan 01-01 populates them with real SQL
    let schema_sql = include_str!("../src/db/migrations/001_initial_schema.sql");
    if !schema_sql.trim().starts_with("-- Placeholder") {
        conn.execute_batch(schema_sql)
            .await
            .expect("Failed to apply schema migration");
    }

    let triggers_sql = include_str!("../src/db/migrations/002_audit_triggers.sql");
    if !triggers_sql.trim().starts_with("-- Placeholder") {
        conn.execute_batch(triggers_sql)
            .await
            .expect("Failed to apply audit trigger migration");
    }

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
/// NOTE: Only works after Plan 01-01 populates the real schema migration.
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
