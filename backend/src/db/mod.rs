use std::time::Duration;

use anyhow::Result;

use crate::config::Config;

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
    conn.execute("PRAGMA foreign_keys = ON;", ()).await?;
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
    conn.execute("PRAGMA foreign_keys = ON;", ()).await?;

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
                "INSERT INTO _migrations (name, applied_at) VALUES (?1, ?2)",
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
