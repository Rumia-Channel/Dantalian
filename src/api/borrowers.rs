use crate::AppState;
use crate::db::Borrower;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

type ApiError = (StatusCode, Json<serde_json::Value>);

#[derive(Deserialize)]
pub struct CreateBorrowerRequest {
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateBorrowerRequest {
    pub name: Option<String>,
    pub notes: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<Borrower>>, ApiError> {
    let db = state.db.clone();
    let borrowers = tokio::task::spawn_blocking(move || db.list_borrowers())
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok(Json(borrowers))
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateBorrowerRequest>,
) -> Result<(StatusCode, Json<Borrower>), ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Name is required"})),
        ));
    }
    let name = req.name.trim().to_string();
    let notes = req.notes.clone();
    let db = state.db.clone();
    let borrower = tokio::task::spawn_blocking(move || {
        db.insert_borrower(&name, notes.as_deref())
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Internal error"})),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok((StatusCode::CREATED, Json(borrower)))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateBorrowerRequest>,
) -> Result<StatusCode, ApiError> {
    if let Some(ref name) = req.name {
        if name.trim().is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Name is required"})),
            ));
        }
    }
    let name = req.name.clone();
    let notes = req.notes.clone();
    let db = state.db.clone();
    let ok = tokio::task::spawn_blocking(move || db.update_borrower(id, name.as_deref(), notes.as_deref()))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Borrower not found"})),
        ))
    }
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let db = state.db.clone();
    let ok = tokio::task::spawn_blocking(move || db.delete_borrower(id))
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal error"})),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Borrower not found"})),
        ))
    }
}
