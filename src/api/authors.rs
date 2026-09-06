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

#[derive(serde::Deserialize)]
pub struct MergeAuthorsRequest {
    pub survivor_id: i64,
    #[serde(default)]
    pub duplicate_ids: Vec<i64>,
}

/// Merge duplicate author rows into one survivor so every list groups by a
/// single author ID. Reassigns book/CD/track links, backfills identity
/// fields, deletes the duplicates.
pub async fn merge(
    State(state): State<AppState>,
    Json(request): Json<MergeAuthorsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = AuthorService::new(NativeAuthorRepository::new(state.db));
    service
        .merge(request.survivor_id, &request.duplicate_ids)
        .await
        .map(|merged| Json(serde_json::json!({"merged": merged})))
        .map_err(error_response)
}
