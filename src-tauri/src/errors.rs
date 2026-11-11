use serde::Serialize;
use std::{error::Error, fmt};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

impl AppError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details,
        }
    }

    pub fn io(context: &str, error: impl fmt::Display) -> Self {
        Self::new("filesystem_error", context, Some(error.to_string()))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for AppError {}
