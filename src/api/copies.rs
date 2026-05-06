use crate::AppState;
use crate::db::{CopyWithStatus, LendingRecord, NewLendingRecord};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

type ApiError = (StatusCode, Json<serde_json::Value>);

#[derive(Deserialize)]
pub struct CreateCopyRequest {
    pub copy_type: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateCopyRequest {
    pub copy_type: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

pub async fn list_copies(
    State(state): State<AppState>,
    Path(book_id): Path<i64>,
) -> Result<Json<Vec<CopyWithStatus>>, ApiError> {
    let db = state.db.clone();
    let copies = tokio::task::spawn_blocking(move || db.list_copies(book_id))
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
    Ok(Json(copies))
}

pub async fn create_copy(
    State(state): State<AppState>,
    Path(book_id): Path<i64>,
    Json(req): Json<CreateCopyRequest>,
) -> Result<(StatusCode, Json<crate::db::Copy>), ApiError> {
    let copy_type = req.copy_type.clone().unwrap_or_else(|| "physical".to_string());
    let location = req.location.clone();
    let notes = req.notes.clone();
    let db = state.db.clone();
    let copy = tokio::task::spawn_blocking(move || {
        db.insert_copy(
            book_id,
            &copy_type,
            location.as_deref(),
            notes.as_deref(),
        )
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
    Ok((StatusCode::CREATED, Json(copy)))
}

pub async fn update_copy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateCopyRequest>,
) -> Result<StatusCode, ApiError> {
    let copy_type = req.copy_type.clone();
    let location = req.location.clone();
    let notes = req.notes.clone();
    let db = state.db.clone();
    let ok = tokio::task::spawn_blocking(move || {
        db.update_copy(
            id,
            copy_type.as_deref(),
            location.as_deref(),
            notes.as_deref(),
        )
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
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Copy not found"})),
        ))
    }
}

pub async fn delete_copy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let db = state.db.clone();
    let ok = tokio::task::spawn_blocking(move || db.delete_copy(id))
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
            Json(serde_json::json!({"error": "Copy not found"})),
        ))
    }
}

pub async fn lend_copy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<NewLendingRecord>,
) -> Result<StatusCode, ApiError> {
    let borrower_id = req.borrower_id;
    let due_date = req.due_date.clone();
    let notes = req.notes.clone();
    let db = state.db.clone();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    tokio::task::spawn_blocking(move || {
        db.lend_copy(id, borrower_id, &today, due_date.as_deref(), notes.as_deref())
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Internal error"})),
        )
    })?
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("InvalidType") {
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "Copy is already lent"})),
            )
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": msg})),
            )
        }
    })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn return_copy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let db = state.db.clone();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let ok = tokio::task::spawn_blocking(move || db.return_copy(id, &today))
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
            Json(serde_json::json!({"error": "Copy not lent or not found"})),
        ))
    }
}

pub async fn get_lending_history(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<LendingRecord>>, ApiError> {
    let db = state.db.clone();
    let records = tokio::task::spawn_blocking(move || db.get_lending_history(id))
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
    Ok(Json(records))
}
