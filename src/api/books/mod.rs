pub mod crud;
pub mod register;

use axum::Json;
use axum::http::StatusCode;

pub type ApiError = (StatusCode, Json<serde_json::Value>);

pub use crud::*;
pub use register::*;
