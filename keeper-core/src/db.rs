use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect(url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(url)
        .await
        .context("Connecting to Postgres")
}

pub async fn ping(pool: &PgPool) -> Result<()> {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .context("Database Ping")?;
    Ok(())
}
