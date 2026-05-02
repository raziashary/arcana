use thiserror::Error;

#[derive(Debug, Error)]
pub enum SharedError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("TOML error: {0}")]
    Toml(String),

    #[error("{0}")]
    Other(String),
}

impl From<toml::de::Error> for SharedError {
    fn from(e: toml::de::Error) -> Self {
        SharedError::Toml(e.to_string())
    }
}

impl From<toml::ser::Error> for SharedError {
    fn from(e: toml::ser::Error) -> Self {
        SharedError::Toml(e.to_string())
    }
}

impl From<anyhow::Error> for SharedError {
    fn from(e: anyhow::Error) -> Self {
        SharedError::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SharedError>;
