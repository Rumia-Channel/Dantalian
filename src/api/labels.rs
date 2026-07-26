use crate::AppState;
use crate::db::Label;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateLabelRequest {
    pub name: String,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateLabelRequest>,
) -> Result<(StatusCode, Json<Label>), ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Label name is required"})),
        ));
    }
    let label = state.db.get_or_create_label(req.name.trim()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok((StatusCode::CREATED, Json(label)))
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Label>>, StatusCode> {
    let db = state.db.clone();
    let labels = tokio::task::spawn_blocking(move || db.list_labels())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(labels))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateLabelRequest>,
) -> Result<StatusCode, ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Label name is required"})),
        ));
    }
    if state.db.rename_label(id, req.name.trim()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Label not found"})),
        ))
    }
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    if state
        .db
        .delete_label(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
