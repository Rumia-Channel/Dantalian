use axum::{Json, http::StatusCode};

use crate::application::error::AppError;

pub(crate) type ApiError = (StatusCode, Json<serde_json::Value>);

pub(crate) fn error_response(error: AppError) -> ApiError {
    let status = status_for_error(&error);
    (
        status,
        Json(serde_json::json!({ "error": error.to_string() })),
    )
}

pub(crate) fn status_for_error(error: &AppError) -> StatusCode {
    match error {
        AppError::Validation(_) => StatusCode::BAD_REQUEST,
        AppError::NotFound => StatusCode::NOT_FOUND,
        AppError::Conflict(_) => StatusCode::CONFLICT,
        AppError::Database(_) | AppError::Storage(_) | AppError::Internal(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
