use crate::AppState;
use crate::db::BookWithAuthors;
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

#[derive(Deserialize)]
pub struct UpdateBookRequest {
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub alternative: Option<String>,
    pub alternative_transcription: Option<String>,
    pub volume: Option<String>,
    pub volume_transcription: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub jpno: Option<String>,
    pub ndl_url: Option<String>,
    pub series_id: Option<Option<i64>>,
    pub series_number: Option<i64>,
    pub grand_series_id: Option<Option<i64>>,
}

pub async fn update_book(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateBookRequest>,
) -> Result<StatusCode, ApiError> {
    if req.title.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Title is required"})),
        ));
    }
    let series_id = req.series_id.unwrap_or(None);
    let grand_series_id = req.grand_series_id.unwrap_or(None);

    if let Some(gs_id) = grand_series_id {
        if let Ok(Some(old_gs)) = state.db.get_book_grand_series(id) {
            let _ = state.db.remove_grand_series_item(old_gs.id, "book", id);
        }
        if gs_id != 0 {
            let _ = state.db.add_grand_series_item(gs_id, "book", id);
        }
    }

    state
        .db
        .update_book(
            id,
            req.title.trim(),
            req.publisher.as_deref(),
            req.publish_date.as_deref(),
            req.description.as_deref(),
            req.title_transcription.as_deref(),
            req.series_title.as_deref(),
            req.series_title_transcription.as_deref(),
            req.alternative.as_deref(),
            req.alternative_transcription.as_deref(),
            req.volume.as_deref(),
            req.volume_transcription.as_deref(),
            req.price.as_deref(),
            req.extent.as_deref(),
            req.jpno.as_deref(),
            req.ndl_url.as_deref(),
            series_id,
            req.series_number,
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct UpdateAuthorRequest {
    pub name: String,
    pub transcription: Option<String>,
    pub ndl_id: Option<String>,
}

pub async fn update_author(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateAuthorRequest>,
) -> Result<StatusCode, ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Name is required"})),
        ));
    }
    state
        .db
        .update_author(id, req.name.trim(), req.transcription.as_deref(), req.ndl_id.as_deref())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_authors(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::db::Author>>, StatusCode> {
    state
        .db
        .list_authors()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Deserialize)]
pub struct CreateAuthorRequest {
    pub name: String,
    pub transcription: Option<String>,
    pub ndl_id: Option<String>,
}

pub async fn create_author(
    State(state): State<AppState>,
    Json(req): Json<CreateAuthorRequest>,
) -> Result<(StatusCode, Json<crate::db::Author>), ApiError> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Name is required"})),
        ));
    }
    let author = state
        .db
        .create_author(req.name.trim(), req.transcription.as_deref(), req.ndl_id.as_deref())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok((StatusCode::CREATED, Json(author)))
}

pub async fn remove_book_author(
    State(state): State<AppState>,
    Path((book_id, author_id)): Path<(i64, i64)>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .remove_book_author(book_id, author_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
