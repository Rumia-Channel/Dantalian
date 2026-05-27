pub mod crud;
pub mod register;

use axum::http::StatusCode;
use axum::Json;

pub type ApiError = (StatusCode, Json<serde_json::Value>);

pub use crud::*;
pub use register::*;
