use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

use crate::error::VaultError;

/// Open (or create) the SQLite database and run pending migrations.
pub async fn open(db_path: &Path) -> Result<SqlitePool, VaultError> {
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .pragma("foreign_keys", "ON")
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;

    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(|e| VaultError::Database(e.to_string()))?;

    Ok(pool)
}
