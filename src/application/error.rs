use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    NotFound,
    Validation(String),
    Conflict(String),
    Database(String),
    Storage(String),
    Internal(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "not found"),
            Self::Validation(message)
            | Self::Conflict(message)
            | Self::Database(message)
            | Self::Storage(message)
            | Self::Internal(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AppError {}
