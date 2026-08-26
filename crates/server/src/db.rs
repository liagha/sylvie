use std::path::Path;
use std::str::FromStr;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use sylvie_core::error::Error;

fn internal<E: std::fmt::Display>(error: E) -> Error {
    Error::Internal(error.to_string())
}

pub async fn open(path: &Path) -> Result<SqlitePool, Error> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await.map_err(internal)?;
    }
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .map_err(internal)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .map_err(internal)?;
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .map_err(internal)?;
    Ok(pool)
}
