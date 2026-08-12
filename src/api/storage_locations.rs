use crate::{
    AppState,
    adapters::native_storage_location::NativeStorageLocationRepository,
    api::error::{ApiError, error_response},
    application::storage_location::StorageLocationService,
    domain::storage_location::{CreateStorageLocation, StorageLocation, UpdateStorageLocation},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateStorageLocation>,
) -> Result<(StatusCode, Json<StorageLocation>), ApiError> {
    let service = StorageLocationService::new(NativeStorageLocationRepository::new(state.db));
    service
        .create(&request.name, request.parent_id)
        .await
        .map(|location| (StatusCode::CREATED, Json(location)))
        .map_err(error_response)
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<StorageLocation>>, ApiError> {
    let service = StorageLocationService::new(NativeStorageLocationRepository::new(state.db));
    service.list().await.map(Json).map_err(error_response)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateStorageLocation>,
) -> Result<StatusCode, ApiError> {
    let service = StorageLocationService::new(NativeStorageLocationRepository::new(state.db));
    service
        .update(id, request.name.as_deref(), request.parent_id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let service = StorageLocationService::new(NativeStorageLocationRepository::new(state.db));
    service
        .delete(id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}
