use crate::AppState;
use crate::db::BookWithAuthors;
use crate::external;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub isbn: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub book: BookWithAuthors,
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
        let authors = state.db.get_book_authors(existing.id).unwrap_or_default();
        return Ok((
            StatusCode::OK,
            Json(RegisterResponse {
                book: BookWithAuthors { book: existing, authors },
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

    let mut authors = Vec::new();
    for a in &new_book.authors {
        let aid = state.db.insert_author(
            a.ndl_id.as_deref(),
            &a.name,
            a.transcription.as_deref(),
        ).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        state.db.add_book_author(book.id, aid).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        let full = state.db.get_author_by_id(aid).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        if let Some(author) = full {
            authors.push(author);
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            book: BookWithAuthors { book, authors },
            source,
        }),
    ))
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<BookWithAuthors>>, StatusCode> {
    let books = state
        .db
        .list_books()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut result = Vec::new();
    for book in books {
        let authors = state.db.get_book_authors(book.id).unwrap_or_default();
        result.push(BookWithAuthors { book, authors });
    }

    Ok(Json(result))
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

#[derive(Deserialize)]
pub struct SetSeriesRequest {
    pub series_id: Option<i64>,
}

pub async fn set_series(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<SetSeriesRequest>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .set_book_series(id, req.series_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct AuthorQuery {
    pub ndl_id: Option<String>,
}

pub async fn get_author(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<crate::db::Author>, StatusCode> {
    state
        .db
        .get_author_by_id(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn search_author(
    State(state): State<AppState>,
    Query(q): Query<AuthorQuery>,
) -> Result<Json<crate::db::Author>, StatusCode> {
    let ndl_id = q.ndl_id.ok_or(StatusCode::BAD_REQUEST)?;
    state
        .db
        .get_author_by_ndl_id(&ndl_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
