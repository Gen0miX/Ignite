use serde::{Serialize, Serializer};

/// Error type shared by backend modules and returned by Tauri commands.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("network error: {message}")]
    Network {
        status: Option<u16>,
        message: String,
    },
    #[error("secret store error: {0}")]
    Secret(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type AppResult<T> = Result<T, AppError>;

// Commands send errors to the frontend as plain messages; never include secrets in them.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
