//! Phase 9 — E2E DB seed binary. Gated by Cargo feature "seed-e2e".
//!
//! Reuses production code paths so password hashes match the login flow:
//! - cronometrix_api::config::Config::from_env (reads DEVICE_CREDS_KEY, JWT_SECRET, etc.)
//! - cronometrix_api::db::init_db (runs all migrations)
//! - cronometrix_api::auth::service::hash_password (argon2id, identical params)
//! - cronometrix_api::devices::crypto::encrypt_password (AES-256-GCM, same key)
//!
//! Refuses to run without CRONOMETRIX_E2E=true. Idempotent — INSERT OR IGNORE.
//!
//! Seeded data:
//!   Users (e2e):  e2e_admin / e2e_supervisor / e2e_viewer
//!   Users (demo): demo_admin / demo_super / demo_viewer (shared password)
//!   Departments:  dept-prod, dept-admin, dept-rrhh
//!   Employees:    6 employees, 2 per department
//!   Devices:      dev-entry on 127.0.0.1:4400, dev-exit on 127.0.0.1:4401
//!
//! Implementation note — use tuple params (not libsql::params![]) for inserts
//! that mix &str and String values. libsql::params! produces [Result<Value>; N]
//! which requires all expressions to coerce to the same array element type;
//! the tuple form goes through per-element IntoValue conversion and avoids the
//! silent-ignore behaviour observed with mixed &str/String in the same macro call.

use cronometrix_api::{auth, config::Config, db, devices::crypto as dev_crypto};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("CRONOMETRIX_E2E").as_deref() != Ok("true") {
        eprintln!("seed_e2e refuses to run without CRONOMETRIX_E2E=true");
        std::process::exit(2);
    }

    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env()?;
    let db = db::init_db(&config).await?;
    let conn = db.connect()?;

    // ----- Users -----
    // Passwords use production argon2id hash_password so login works identically.
    // Tuple params used (not libsql::params![]) to avoid type-coercion silent-ignore
    // when mixing &str loop vars with owned String hash values.
    let admin_hash = auth::service::hash_password("e2e-admin-pass")?;
    let supervisor_hash = auth::service::hash_password("e2e-supervisor-pass")?;
    let viewer_hash = auth::service::hash_password("e2e-viewer-pass")?;

    // Schema (001_initial_schema.sql): id, username, full_name, password_hash, role,
    // refresh_token_hash, status, deleted_at, version, created_at, updated_at
    conn.execute(
        "INSERT OR IGNORE INTO users \
         (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'admin', 'active', 1, unixepoch(), unixepoch())",
        ("e2e-admin-id", "e2e_admin", "E2E Admin", admin_hash.as_str()),
    )
    .await
    .map_err(|e| anyhow::anyhow!("users insert failed for e2e_admin: {}", e))?;

    conn.execute(
        "INSERT OR IGNORE INTO users \
         (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'supervisor', 'active', 1, unixepoch(), unixepoch())",
        ("e2e-supervisor-id", "e2e_supervisor", "E2E Supervisor", supervisor_hash.as_str()),
    )
    .await
    .map_err(|e| anyhow::anyhow!("users insert failed for e2e_supervisor: {}", e))?;

    conn.execute(
        "INSERT OR IGNORE INTO users \
         (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'viewer', 'active', 1, unixepoch(), unixepoch())",
        ("e2e-viewer-id", "e2e_viewer", "E2E Viewer", viewer_hash.as_str()),
    )
    .await
    .map_err(|e| anyhow::anyhow!("users insert failed for e2e_viewer: {}", e))?;

    // ----- Demo users (shared password — for live demo handoff, not e2e) -----
    let demo_pass = "dSQBALuQgXWZp6Oo";
    let demo_hash = auth::service::hash_password(demo_pass)?;
    for (id, username, full_name, role) in [
        ("demo-admin-id",  "demo_admin",  "Demo Admin",      "admin"),
        ("demo-super-id",  "demo_super",  "Demo Supervisor", "supervisor"),
        ("demo-viewer-id", "demo_viewer", "Demo Viewer",     "viewer"),
    ] {
        conn.execute(
            &format!(
                "INSERT OR IGNORE INTO users \
                 (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, '{role}', 'active', 1, unixepoch(), unixepoch())"
            ),
            (id, username, full_name, demo_hash.as_str()),
        )
        .await
        .map_err(|e| anyhow::anyhow!("users insert failed for {}: {}", username, e))?;
    }

    // ----- Departments -----
    // Schema (001 + 012): id, name, base_salary_cents, shift_start_time, shift_end_time,
    // lunch_mode, lunch_duration_min, status, version, created_at, updated_at,
    // shift_type, is_overnight_shift, ordinary_daily_minutes
    for (id, name) in [
        ("dept-prod",  "Producción"),
        ("dept-admin", "Administración"),
        ("dept-rrhh",  "Recursos Humanos"),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO departments \
             (id, name, base_salary_cents, shift_start_time, shift_end_time, \
              lunch_mode, lunch_duration_min, shift_type, is_overnight_shift, \
              ordinary_daily_minutes, status, version, created_at, updated_at) \
             VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'day', 0, 480, \
             'active', 1, unixepoch(), unixepoch())",
            (id, name),
        )
        .await
        .map_err(|e| anyhow::anyhow!("departments insert failed for {}: {}", id, e))?;
    }

    // ----- Employees (6 — 2 per department, varied names + salaries for filter tests) -----
    // Schema (001 + 015 + 018): id, employee_code, name, department_id, status, deleted_at,
    // version, created_at, updated_at, position, hire_date, base_salary_cents
    //
    // base_salary_cents spread across $30..$80 USD (3000..8000 cents) to validate
    // payroll math + reports columns with realistic-but-bounded values.
    for (id, code, name, dept_id, salary_cents) in [
        ("emp-ana",    "EMP001", "Ana Pérez",       "dept-prod",  3000_i64),
        ("emp-luis",   "EMP002", "Luis García",      "dept-prod",  4000_i64),
        ("emp-maria",  "EMP003", "María López",      "dept-admin", 5000_i64),
        ("emp-pedro",  "EMP004", "Pedro Ramírez",    "dept-admin", 6000_i64),
        ("emp-carmen", "EMP005", "Carmen Silva",     "dept-rrhh",  7000_i64),
        ("emp-jose",   "EMP006", "José Hernández",   "dept-rrhh",  8000_i64),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO employees \
             (id, employee_code, name, department_id, status, version, created_at, updated_at, position, base_salary_cents) \
             VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch(), '', ?5)",
            (id, code, name, dept_id, salary_cents),
        )
        .await
        .map_err(|e| anyhow::anyhow!("employees insert failed for {}: {}", id, e))?;
    }

    // ----- Devices (2 — pointed at mock_hikvision) -----
    // Schema (003_devices.sql): id, name, ip, port, scheme, username, encrypted_password,
    // direction, allow_insecure_tls, connection_state, status, version, created_at, updated_at
    //
    // Two devices use different ports (4400 / 4401) to avoid the partial unique index:
    //   idx_devices_ip_port_active ON devices(ip, port) WHERE status = 'active'
    // mock_hikvision public port is 4400; admin port is 4401. The exit device connects
    // to the admin port — it will receive connection errors from mock_hikvision for ISAPI
    // traffic, which is acceptable for E2E specs that only need the entry device.
    //
    // Passwords use the production AES-256-GCM encrypt so the backend can decrypt them.
    let mock_pass_enc = dev_crypto::encrypt_password("mock-device-pass", &config.device_creds_key)?;

    // Use 2-tuple (id, enc_password) — tuple of two &str is supported by IntoParams.
    conn.execute(
        "INSERT OR IGNORE INTO devices \
         (id, name, ip, port, scheme, username, encrypted_password, direction, \
          allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, 'Entrada Principal', '127.0.0.1', 4400, 'http', 'admin', \
         ?2, 'entry', 1, 'offline', 'active', 1, unixepoch(), unixepoch())",
        ("dev-entry", mock_pass_enc.as_str()),
    )
    .await
    .map_err(|e| anyhow::anyhow!("devices insert failed for dev-entry: {}", e))?;

    conn.execute(
        "INSERT OR IGNORE INTO devices \
         (id, name, ip, port, scheme, username, encrypted_password, direction, \
          allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, 'Salida Principal', '127.0.0.1', 4401, 'http', 'admin', \
         ?2, 'exit', 1, 'offline', 'active', 1, unixepoch(), unixepoch())",
        ("dev-exit", mock_pass_enc.as_str()),
    )
    .await
    .map_err(|e| anyhow::anyhow!("devices insert failed for dev-exit: {}", e))?;

    tracing::info!("seed_e2e: seeded 6 users (3 e2e + 3 demo), 3 departments, 6 employees, 2 devices");
    println!("seed_e2e: complete");
    Ok(())
}
