use crate::{
    AppState,
    adapters::native_label::NativeLabelRepository,
    application::{error::AppError, label::LabelService},
    domain::label::{CreateLabel, Label, RenameLabel},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

type ApiError = (StatusCode, Json<serde_json::Value>);

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateLabel>,
) -> Result<(StatusCode, Json<Label>), ApiError> {
    let service = LabelService::new(NativeLabelRepository::new(state.db));
    service
        .create(&request.name)
        .await
        .map(|label| (StatusCode::CREATED, Json(label)))
        .map_err(error_response)
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Label>>, StatusCode> {
    let service = LabelService::new(NativeLabelRepository::new(state.db));
    service
        .list()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<RenameLabel>,
) -> Result<StatusCode, ApiError> {
    let service = LabelService::new(NativeLabelRepository::new(state.db));
    service
        .rename(id, &request.name)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    let service = LabelService::new(NativeLabelRepository::new(state.db));
    match service.delete(id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(AppError::NotFound) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn error_response(error: AppError) -> ApiError {
    let status = match error {
        AppError::Validation(_) => StatusCode::BAD_REQUEST,
        AppError::NotFound => StatusCode::NOT_FOUND,
        AppError::Conflict(_) => StatusCode::CONFLICT,
        AppError::Database(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(serde_json::json!({ "error": error.to_string() })),
    )
}
