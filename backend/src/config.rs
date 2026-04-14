use anyhow::{Context, Result};

/// Configuration loaded from environment variables at startup.
/// Panics if required variables (JWT_SECRET) are missing or invalid.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: String,
    pub turso_url: String,
    pub turso_token: String,
    pub jwt_secret: String,
    pub server_host: String,
    pub server_port: u16,
    pub turso_sync_interval_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_path = std::env::var("CRONOMETRIX_DB_PATH")
            .unwrap_or_else(|_| "cronometrix.db".to_string());

        let turso_url = std::env::var("TURSO_DATABASE_URL").unwrap_or_default();
        let turso_token = std::env::var("TURSO_AUTH_TOKEN").unwrap_or_default();

        let jwt_secret = std::env::var("JWT_SECRET")
            .context("JWT_SECRET environment variable is required")?;

        if jwt_secret.len() < 32 {
            anyhow::bail!(
                "JWT_SECRET must be at least 32 characters long (got {})",
                jwt_secret.len()
            );
        }

        let server_host = std::env::var("SERVER_HOST")
            .unwrap_or_else(|_| "0.0.0.0".to_string());

        let server_port = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3001".to_string())
            .parse::<u16>()
            .context("SERVER_PORT must be a valid port number")?;

        let turso_sync_interval_secs = std::env::var("TURSO_SYNC_INTERVAL")
            .unwrap_or_else(|_| "300".to_string())
            .parse::<u64>()
            .context("TURSO_SYNC_INTERVAL must be a valid number of seconds")?;

        Ok(Config {
            database_path,
            turso_url,
            turso_token,
            jwt_secret,
            server_host,
            server_port,
            turso_sync_interval_secs,
        })
    }

    /// Returns true if Turso remote sync is configured (URL is non-empty).
    pub fn has_turso(&self) -> bool {
        !self.turso_url.is_empty()
    }
}
