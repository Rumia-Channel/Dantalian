use crate::AppState;
use crate::db::GrandSeriesWithItems;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateGrandSeriesRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct RenameGrandSeriesRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct AddGrandSeriesItemRequest {
    pub item_type: String,
    pub item_id: i64,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateGrandSeriesRequest>,
) -> Result<(StatusCode, Json<crate::db::GrandSeries>), ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Name is required"})),
        ));
    }
    let gs = state.db.create_grand_series(req.name.trim()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok((StatusCode::CREATED, Json(gs)))
}

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<GrandSeriesWithItems>>, StatusCode> {
    let db = state.db.clone();
    let grand_series = tokio::task::spawn_blocking(move || db.list_grand_series())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(grand_series))
}

pub async fn rename(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<RenameGrandSeriesRequest>,
) -> Result<StatusCode, ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Name is required"})),
        ));
    }
    state
        .db
        .rename_grand_series(id, req.name.trim())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    if state
        .db
        .delete_grand_series(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn add_item(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<AddGrandSeriesItemRequest>,
) -> Result<StatusCode, ApiError> {
    if req.item_type != "series" && req.item_type != "book" && req.item_type != "cd" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "item_type must be 'series', 'book', or 'cd'"})),
        ));
    }
    state
        .db
        .add_grand_series_item(id, &req.item_type, req.item_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_item(
    State(state): State<AppState>,
    Path((id, item_type, item_id)): Path<(i64, String, i64)>,
) -> Result<StatusCode, ApiError> {
    if item_type != "series" && item_type != "book" && item_type != "cd" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "item_type must be 'series', 'book', or 'cd'"})),
        ));
    }
    state
        .db
        .remove_grand_series_item(id, &item_type, item_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}
