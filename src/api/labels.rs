use crate::{
    AppState,
    adapters::native_label::NativeLabelRepository,
    api::error::{ApiError, error_response},
    application::label::LabelService,
    domain::label::{CreateLabel, Label, RenameLabel},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

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

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Label>>, ApiError> {
    let service = LabelService::new(NativeLabelRepository::new(state.db));
    service.list().await.map(Json).map_err(error_response)
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
) -> Result<StatusCode, ApiError> {
    let service = LabelService::new(NativeLabelRepository::new(state.db));
    service
        .delete(id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}
