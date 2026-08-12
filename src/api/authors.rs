use crate::{
    AppState,
    adapters::native_author::NativeAuthorRepository,
    api::error::{ApiError, error_response},
    application::author::AuthorService,
    domain::author::{Author, CreateAuthor, UpdateAuthor},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Author>>, ApiError> {
    let service = AuthorService::new(NativeAuthorRepository::new(state.db));
    service.list().await.map(Json).map_err(error_response)
}

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateAuthor>,
) -> Result<(StatusCode, Json<Author>), ApiError> {
    let service = AuthorService::new(NativeAuthorRepository::new(state.db));
    service
        .create(
            &request.name,
            request.transcription.as_deref(),
            request.ndl_id.as_deref(),
        )
        .await
        .map(|author| (StatusCode::CREATED, Json(author)))
        .map_err(error_response)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Author>, ApiError> {
    let service = AuthorService::new(NativeAuthorRepository::new(state.db));
    service.get(id).await.map(Json).map_err(error_response)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateAuthor>,
) -> Result<StatusCode, ApiError> {
    let service = AuthorService::new(NativeAuthorRepository::new(state.db));
    service
        .update(
            id,
            &request.name,
            request.transcription.as_deref(),
            request.ndl_id.as_deref(),
        )
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let service = AuthorService::new(NativeAuthorRepository::new(state.db));
    service
        .delete(id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}
