use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("I/O operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("application error: {0}")]
    Other(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<AppError> for CommandError {
    fn from(value: AppError) -> Self {
        let code = match value {
            AppError::Database(_) => "database_error",
            AppError::Io(_) => "io_error",
            AppError::InvalidRequest(_) => "invalid_request",
            AppError::Other(_) => "application_error",
        };
        Self {
            code,
            message: value.to_string(),
        }
    }
}

impl From<rusqlite::Error> for CommandError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Database(value).into()
    }
}
