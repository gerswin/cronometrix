use std::time::Duration;

use anyhow::Result;

use crate::config::Config;

pub mod write_queue;

/// Embedded SQL migrations — applied in order at startup.
/// Each tuple is (migration_name, sql_content).
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial_schema",
        include_str!("migrations/001_initial_schema.sql"),
    ),
    (
        "002_audit_triggers",
        include_str!("migrations/002_audit_triggers.sql"),
    ),
    (
        "003_devices",
        include_str!("migrations/003_devices.sql"),
    ),
    (
        "004_attendance_events",
        include_str!("migrations/004_attendance_events.sql"),
    ),
    (
        "005_command_audit_log",
        include_str!("migrations/005_command_audit_log.sql"),
    ),
    (
        "006_devices_audit_triggers",
        include_str!("migrations/006_devices_audit_triggers.sql"),
    ),
    (
        "007_daily_records",
        include_str!("migrations/007_daily_records.sql"),
    ),
    (
        "008_daily_record_anomalies",
        include_str!("migrations/008_daily_record_anomalies.sql"),
    ),
    (
        "009_daily_record_overrides",
        include_str!("migrations/009_daily_record_overrides.sql"),
    ),
    (
        "010_leaves",
        include_str!("migrations/010_leaves.sql"),
    ),
    (
        "011_phase3_audit_triggers",
        include_str!("migrations/011_phase3_audit_triggers.sql"),
    ),
    (
        "012_shift_type_to_departments",
        include_str!("migrations/012_shift_type_to_departments.sql"),
    ),
    (
        "013_tenant_info",
        include_str!("migrations/013_tenant_info.sql"),
    ),
    (
        "014_phase5_audit_triggers",
        include_str!("migrations/014_phase5_audit_triggers.sql"),
    ),
    (
        "015_employees_position_hire_date",
        include_str!("migrations/015_employees_position_hire_date.sql"),
    ),
    (
        "016_enrollments",
        include_str!("migrations/016_enrollments.sql"),
    ),
    (
        "017_phase7_audit_triggers",
        include_str!("migrations/017_phase7_audit_triggers.sql"),
    ),
    (
        "018_employees_base_salary",
        include_str!("migrations/018_employees_base_salary.sql"),
    ),
];

/// Initialize the database. If Turso URL is configured, builds an embedded
/// replica with cloud sync. Otherwise falls back to local-only mode (for
/// development without Turso credentials).
pub async fn init_db(config: &Config) -> Result<libsql::Database> {
    if config.has_turso() {
        init_db_remote(config).await
    } else {
        init_db_local(config).await
    }
}

/// Initialize local-only SQLite database (no Turso sync).
/// Used when TURSO_DATABASE_URL is not set — enables development without credentials.
pub async fn init_db_local(config: &Config) -> Result<libsql::Database> {
    tracing::info!(
        "Starting in local-only mode (no Turso URL configured). \
         Database: {}",
        config.database_path
    );

    let db = libsql::Builder::new_local(&config.database_path)
        .build()
        .await?;

    let conn = db.connect()?;
    // SQLite contention is expected in dev because the supervisor, watchdog,
    // HTTP handlers, and background workers can all touch the same file.
    // WAL + a short busy timeout makes transient writer collisions wait instead
    // of immediately surfacing as `database is locked`.
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; \
         PRAGMA journal_mode = WAL; \
         PRAGMA synchronous = NORMAL; \
         PRAGMA busy_timeout = 5000;",
    )
    .await?;
    run_migrations(&conn).await?;

    Ok(db)
}

/// Initialize embedded replica database with Turso cloud sync.
async fn init_db_remote(config: &Config) -> Result<libsql::Database> {
    tracing::info!(
        "Connecting to Turso remote replica. Local path: {}",
        config.database_path
    );

    let db = libsql::Builder::new_remote_replica(
        &config.database_path,
        config.turso_url.clone(),
        config.turso_token.clone(),
    )
    .sync_interval(Duration::from_secs(config.turso_sync_interval_secs))
    .read_your_writes(true)
    .build()
    .await?;

    let conn = db.connect()?;

    // Apply PRAGMAs on local connection BEFORE sync (Pitfall 3: remote connection
    // does not support PRAGMA statements).
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; \
         PRAGMA journal_mode = WAL; \
         PRAGMA synchronous = NORMAL; \
         PRAGMA busy_timeout = 5000;",
    )
    .await?;

    run_migrations(&conn).await?;

    // Attempt initial Turso sync. If it fails (network down, credentials wrong),
    // log a warning and continue in local-authoritative mode.
    // Per DATA-03: local SQLite is authoritative — cloud is a replica.
    match db.sync().await {
        Ok(rep) => tracing::info!("Turso initial sync complete: {:?}", rep),
        Err(e) => tracing::warn!(
            "Turso initial sync failed (continuing in local-only mode): {}. \
             Data will sync when connectivity is restored via sync_interval.",
            e
        ),
    }

    Ok(db)
}

/// Run all pending migrations against the provided connection.
/// Uses a _migrations tracking table to ensure idempotency.
pub async fn run_migrations(conn: &libsql::Connection) -> Result<()> {
    // Create migrations tracking table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )",
        (),
    )
    .await?;

    for (name, sql) in MIGRATIONS {
        // Skip placeholder SQL (Wave 0 guard)
        if sql.trim().starts_with("-- Placeholder") {
            tracing::debug!("Skipping placeholder migration: {}", name);
            continue;
        }

        // Check if already applied
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM _migrations WHERE name = ?1",
                [*name],
            )
            .await?;

        let count: i64 = rows
            .next()
            .await?
            .map(|row| row.get::<i64>(0).unwrap_or(0))
            .unwrap_or(0);

        if count == 0 {
            conn.execute_batch(sql).await?;
            conn.execute(
                "INSERT OR IGNORE INTO _migrations (name, applied_at) VALUES (?1, ?2)",
                libsql::params![*name, chrono::Utc::now().timestamp()],
            )
            .await?;
            tracing::info!("Applied migration: {}", name);
        } else {
            tracing::debug!("Migration already applied: {}", name);
        }
    }

    Ok(())
}
