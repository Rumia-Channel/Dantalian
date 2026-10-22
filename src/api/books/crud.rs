use crate::AppState;
use crate::api::upload_chunks::{ChunkQuery, StoreResult};
use crate::db::BookWithAuthors;
use crate::ports::object_storage::ObjectStorage;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_extra::extract::Multipart;
use base64::Engine;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};

use super::ApiError;

const BOOK_CURSOR_MASK: i64 = 0x5A17_C0DE;

#[derive(Deserialize)]
pub struct ListBooksQuery {
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

#[derive(serde::Serialize)]
pub struct BooksPage {
    pub items: Vec<BookWithAuthors>,
    pub next_cursor: Option<String>,
}

fn encode_book_cursor(id: i64) -> String {
    format!("c{:x}", id ^ BOOK_CURSOR_MASK)
}

fn decode_book_cursor(value: &str) -> Result<i64, StatusCode> {
    let encoded = value
        .strip_prefix('c')
        .filter(|v| !v.is_empty() && v.len() <= 16)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let encoded = i64::from_str_radix(encoded, 16).map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = encoded ^ BOOK_CURSOR_MASK;
    if id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(id)
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListBooksQuery>,
) -> Result<Json<BooksPage>, StatusCode> {
    let limit = match query.limit {
        Some(value) if (1..=100).contains(&value) => value,
        Some(_) => return Err(StatusCode::BAD_REQUEST),
        None => 50,
    };
    let cursor = match query.cursor.as_deref() {
        Some(value) => Some(decode_book_cursor(value)?),
        None => None,
    };

    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        // limit+1 件取得して次ページの有無を判定する
        let mut books = db.list_books_page(limit + 1, cursor)?;
        let has_more = books.len() > limit;
        books.truncate(limit);
        let next_cursor = books
            .last()
            .map(|book| encode_book_cursor(book.id))
            .filter(|_| has_more);

        let mut items = Vec::with_capacity(books.len());
        for book in books {
            let authors = db.get_book_authors(book.id).unwrap_or_default();
            let (copies_count, lent_count) = db.get_book_copy_counts(book.id).unwrap_or((0, 0));
            items.push(BookWithAuthors {
                book,
                authors,
                copies_count,
                lent_count,
            });
        }
        Ok::<_, rusqlite::Error>(BooksPage { items, next_cursor })
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result))
}
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<BookWithAuthors>, StatusCode> {
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let book = db.find_by_id(id)?;
        Ok::<_, rusqlite::Error>(book.map(|book| {
            let authors = db.get_book_authors(book.id).unwrap_or_default();
            let (copies_count, lent_count) = db.get_book_copy_counts(book.id).unwrap_or((0, 0));
            BookWithAuthors {
                book,
                authors,
                copies_count,
                lent_count,
            }
        }))
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match result {
        Some(book) => Ok(Json(book)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    let old_hash = state
        .db
        .find_by_id(id)
        .ok()
        .flatten()
        .and_then(|b| b.epub_file_hash);

    let deleted = state
        .db
        .delete_book(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !deleted {
        return Err(StatusCode::NOT_FOUND);
    }

    if let Some(hash) = old_hash {
        match state.db.count_books_by_epub_hash(&hash) {
            Ok(0) => {
                let path = std::path::Path::new(state.epubs_dir.as_str()).join(&hash);
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!(book_id = id, %hash, "Failed to remove orphaned EPUB file: {}", e);
                }
            }
            Ok(n) => {
                tracing::debug!(book_id = id, %hash, remaining = n, "EPUB file kept (still referenced)");
            }
            Err(e) => {
                tracing::warn!(book_id = id, %hash, "Failed to count EPUB references: {}", e);
            }
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct UpdateAuthorOrderRequest {
    pub sort_order: i64,
}

pub async fn update_book_author_order(
    State(state): State<AppState>,
    Path((book_id, author_id)): Path<(i64, i64)>,
    Json(req): Json<UpdateAuthorOrderRequest>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .update_book_author_order(book_id, author_id, req.sort_order)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_book_author(
    State(state): State<AppState>,
    Path((book_id, author_id)): Path<(i64, i64)>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .add_book_author(book_id, author_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
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

pub async fn upload_cover(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(chunk): Query<ChunkQuery>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let book = state
        .db
        .find_by_id(id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Book not found"})),
            )
        })?;
    let chunk_info = chunk.validate().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid upload chunk parameters"})),
        )
    })?;

    let cover_max = crate::api::upload_limit_bytes(
        &state,
        crate::api::KEY_UPLOAD_COVER_MB,
        crate::api::COVER_MAX_BYTES,
    );
    let mut data: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;

    while let Some(mut field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "cover" {
            let ct = field.content_type().unwrap_or("image/jpeg").to_string();
            let read_limit = if chunk_info.is_some() {
                crate::api::upload_chunks::MAX_CHUNK_BYTES
            } else {
                cover_max
            };
            let bytes = crate::api::upload_chunks::read_field_limited(&mut field, read_limit)
                .await
                .map_err(|error| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": error})),
                    )
                })?;
            let bytes = match chunk_info.as_ref() {
                Some(info) => match crate::api::upload_chunks::store_chunk(
                    state.uploads_dir.as_str(),
                    "cover",
                    info.clone(),
                    &bytes,
                    cover_max,
                )
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": "Invalid upload chunk"})),
                    )
                })? {
                    StoreResult::Partial { part, total_parts } => {
                        return Ok(Json(serde_json::json!({
                            "chunked": true,
                            "complete": false,
                            "part": part,
                            "total_parts": total_parts,
                        })));
                    }
                    StoreResult::Complete { path, cleanup_dir } => {
                        let result = std::fs::read(&path).map_err(|e| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": e.to_string()})),
                            )
                        });
                        let _ = std::fs::remove_dir_all(cleanup_dir);
                        result?
                    }
                },
                None => bytes,
            };
            if bytes.len() > cover_max {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        serde_json::json!({"error": format!("File too large (max {}MB)", cover_max / 1024 / 1024)}),
                    ),
                ));
            }
            data = Some(bytes);
            content_type = Some(ct);
        }
    }

    let bytes = data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No file uploaded"})),
        )
    })?;

    let ct = content_type.unwrap_or("image/jpeg".to_string());
    let ext = match ct.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/gif" => "gif",
        _ => "jpg",
    };

    let hash = Sha3_256::digest(&bytes);
    let filename = format!(
        "{}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash),
        ext
    );
    let object_key = format!("images/{filename}");
    state
        .object_storage
        .put_object(&object_key, &ct, &bytes)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": error.to_string()})),
            )
        })?;

    state
        .db
        .update_book_cover_url(id, Some(&filename))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    if let Some(old) = &book.cover_url {
        if old != &filename {
            let _ = state.object_storage.delete(&format!("images/{old}")).await;
        }
    }
    Ok(Json(serde_json::json!({"cover_url": filename})))
}

pub async fn delete_cover(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let book = state
        .db
        .find_by_id(id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Book not found"})),
            )
        })?;

    if let Some(old) = &book.cover_url {
        let _ = state.object_storage.delete(&format!("images/{old}")).await;
    }

    state.db.update_book_cover_url(id, None).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn upload_epub(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(chunk): Query<ChunkQuery>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let book = state
        .db
        .find_by_id(id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Book not found"})),
            )
        })?;
    let chunk_info = chunk.validate().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid upload chunk parameters"})),
        )
    })?;

    let file_max = crate::api::upload_limit_bytes(
        &state,
        crate::api::KEY_UPLOAD_FILE_MB,
        crate::api::EPUB_MAX_BYTES,
    );
    let mut data: Option<Vec<u8>> = None;
    let mut chunked_path: Option<(std::path::PathBuf, std::path::PathBuf)> = None;
    let mut original_name: Option<String> = None;

    while let Some(mut field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "epub" || name == "file" {
            let file_name = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "upload.epub".to_string());
            let read_limit = if chunk_info.is_some() {
                crate::api::upload_chunks::MAX_CHUNK_BYTES
            } else {
                file_max
            };
            let bytes = crate::api::upload_chunks::read_field_limited(&mut field, read_limit)
                .await
                .map_err(|error| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": error})),
                    )
                })?;
            match chunk_info.as_ref() {
                Some(info) => match crate::api::upload_chunks::store_chunk(
                    state.uploads_dir.as_str(),
                    "file",
                    info.clone(),
                    &bytes,
                    file_max,
                )
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": "Invalid upload chunk"})),
                    )
                })? {
                    StoreResult::Partial { part, total_parts } => {
                        return Ok(Json(serde_json::json!({
                            "chunked": true,
                            "complete": false,
                            "part": part,
                            "total_parts": total_parts,
                        })));
                    }
                    StoreResult::Complete { path, cleanup_dir } => {
                        chunked_path = Some((path, cleanup_dir));
                    }
                },
                None => data = Some(bytes),
            }
            original_name = Some(file_name);
        }
    }

    let original_name = original_name.unwrap_or_else(|| "upload.epub".to_string());

    let (saved_name, _ext) = if let Some((path, cleanup_dir)) = chunked_path {
        let result = crate::external::save_uploaded_file_path(
            &path,
            &original_name,
            state.epubs_dir.as_str(),
            file_max,
        );
        let _ = std::fs::remove_dir_all(cleanup_dir);
        result
    } else {
        let bytes = data.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "No file uploaded"})),
            )
        })?;
        crate::external::save_uploaded_file(
            &bytes,
            &original_name,
            state.epubs_dir.as_str(),
            file_max,
        )
    }
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
    })?;

    // Update DB first; only delete the old file after a successful update,
    // and only when no other book still references it.
    let db_result = state
        .db
        .set_book_epub(id, Some(&saved_name), Some(&original_name));

    if let Err(e) = db_result {
        // Best-effort rollback: remove the just-saved file if it is otherwise
        // unreferenced (e.g. brand-new content hash).
        if let Ok(referenced) = state.db.count_books_by_epub_hash(&saved_name) {
            if referenced == 0 {
                let path = std::path::Path::new(state.epubs_dir.as_str()).join(&saved_name);
                if let Err(rm_err) = std::fs::remove_file(&path) {
                    tracing::warn!(
                        book_id = id,
                        %saved_name,
                        "Failed to remove EPUB after DB update failure: {}",
                        rm_err
                    );
                }
            } else {
                tracing::warn!(
                    book_id = id,
                    %saved_name,
                    referenced,
                    "EPUB file kept (referenced by other books) after DB update failure"
                );
            }
        }
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ));
    }

    // DB updated successfully. Clean up the previous EPUB file if it differs
    // from the new one and is no longer referenced by any book.
    if let Some(old) = &book.epub_file_hash {
        if old != &saved_name {
            match state.db.count_books_by_epub_hash(old) {
                Ok(0) => {
                    let old_path = std::path::Path::new(state.epubs_dir.as_str()).join(old);
                    if let Err(e) = std::fs::remove_file(&old_path) {
                        tracing::warn!(
                            book_id = id,
                            %old,
                            "Failed to remove old EPUB file: {}",
                            e
                        );
                    }
                }
                Ok(n) => {
                    tracing::debug!(
                        book_id = id,
                        %old,
                        remaining = n,
                        "Old EPUB file kept (still referenced by other books)"
                    );
                }
                Err(e) => {
                    tracing::warn!(book_id = id, %old, "Failed to count old EPUB references: {}", e);
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "epub_file_hash": saved_name,
        "epub_file_name": original_name,
    })))
}

pub async fn delete_epub(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let book = state
        .db
        .find_by_id(id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Book not found"})),
            )
        })?;

    let old_hash = book.epub_file_hash.clone();

    // Update DB first; only delete the file after a successful update, and
    // only when no other book still references it.
    state.db.set_book_epub(id, None, None).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    if let Some(hash) = old_hash {
        match state.db.count_books_by_epub_hash(&hash) {
            Ok(0) => {
                let path = std::path::Path::new(state.epubs_dir.as_str()).join(&hash);
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!(
                        book_id = id,
                        %hash,
                        "Failed to remove EPUB file after delete: {}",
                        e
                    );
                }
            }
            Ok(n) => {
                tracing::debug!(
                    book_id = id,
                    %hash,
                    remaining = n,
                    "EPUB file kept (still referenced by other books)"
                );
            }
            Err(e) => {
                tracing::warn!(book_id = id, %hash, "Failed to count EPUB references: {}", e);
            }
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
