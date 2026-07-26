use crate::AppState;
use crate::db::Series;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateSeriesRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct RenameSeriesRequest {
    pub name: String,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateSeriesRequest>,
) -> Result<(StatusCode, Json<Series>), ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Series name is required"})),
        ));
    }
    let series = state.db.create_series(req.name.trim()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok((StatusCode::CREATED, Json(series)))
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Series>>, StatusCode> {
    let db = state.db.clone();
    let series = tokio::task::spawn_blocking(move || db.list_series())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(series))
}

pub async fn rename(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<RenameSeriesRequest>,
) -> Result<StatusCode, ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Series name is required"})),
        ));
    }
    state.db.rename_series(id, req.name.trim()).map_err(|e| {
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
        .delete_series(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
