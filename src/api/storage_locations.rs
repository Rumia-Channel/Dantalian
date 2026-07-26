use crate::AppState;
use crate::db::StorageLocation;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateStorageLocationRequest {
    pub name: String,
    pub parent_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateStorageLocationRequest {
    pub name: Option<String>,
    pub parent_id: Option<Option<i64>>,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateStorageLocationRequest>,
) -> Result<(StatusCode, Json<StorageLocation>), ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Location name is required"})),
        ));
    }
    let loc = state
        .db
        .create_storage_location(req.name.trim(), req.parent_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok((StatusCode::CREATED, Json(loc)))
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<StorageLocation>>, StatusCode> {
    let db = state.db.clone();
    let locations = tokio::task::spawn_blocking(move || db.list_storage_locations())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(locations))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateStorageLocationRequest>,
) -> Result<StatusCode, ApiError> {
    if let Some(name) = &req.name {
        if name.trim().is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Location name is required"})),
            ));
        }
        state
            .db
            .rename_storage_location(id, name.trim())
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;
    }
    if let Some(parent_id) = req.parent_id {
        state
            .db
            .set_storage_location_parent(id, parent_id)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    if state
        .db
        .delete_storage_location(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
