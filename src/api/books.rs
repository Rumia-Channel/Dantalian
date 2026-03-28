use crate::AppState;
use crate::db::Book;
use crate::external;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub isbn: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub book: Book,
    pub source: String,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), ApiError> {
    let isbn = req.isbn.replace('-', "").replace(' ', "");
    if isbn.len() != 13 && isbn.len() != 10 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "ISBN must be 10 or 13 digits"})),
        ));
    }

    if let Ok(Some(existing)) = state.db.find_by_isbn(&isbn) {
        return Ok((
            StatusCode::OK,
            Json(RegisterResponse {
                book: existing,
                source: "cache".to_string(),
            }),
        ));
    }

    let new_book = external::lookup_isbn(&state.client, &isbn, &state.images_dir)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e}))))?;

    let Some(new_book) = new_book else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Book not found for this ISBN"})),
        ));
    };

    let source = if new_book.cover_url.is_some() {
        "amazon"
    } else {
        "ndl"
    }
    .to_string();

    let book = state
        .db
        .insert_book(&new_book)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse { book, source }),
    ))
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Book>>, StatusCode> {
    state
        .db
        .list_books()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    if state
        .db
        .delete_book(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
