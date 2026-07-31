use crate::AppState;
use crate::api::upload_chunks::{ChunkQuery, StoreResult};
use crate::db::BookWithAuthors;
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

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<BookWithAuthors>>, StatusCode> {
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let books = db.list_books()?;
        let mut result = Vec::new();
        for book in books {
            let authors = db.get_book_authors(book.id).unwrap_or_default();
            let (copies_count, lent_count) = db.get_book_copy_counts(book.id).unwrap_or((0, 0));
            result.push(BookWithAuthors {
                book,
                authors,
                copies_count,
                lent_count,
            });
        }
        Ok::<_, rusqlite::Error>(result)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result))
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
pub struct SetSeriesRequest {
    pub series_id: Option<Option<i64>>,
    pub series_number: Option<Option<i64>>,
}

pub async fn set_series(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<SetSeriesRequest>,
) -> Result<StatusCode, StatusCode> {
    let existing = state.db.find_by_id(id).ok().flatten();
    let series_id = match req.series_id {
        Some(o) => o,
        None => existing.as_ref().and_then(|b| b.series_id),
    };
    let series_number = match req.series_number {
        Some(o) => o,
        None => existing.as_ref().and_then(|b| b.series_number),
    };
    state
        .db
        .set_book_series(id, series_id, series_number)
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
    pub isbn: Option<String>,
    pub isdn: Option<String>,
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
    pub series_number: Option<Option<i64>>,
    pub grand_series_id: Option<Option<i64>>,
    pub media_type: Option<Option<String>>,
    pub catalog_number: Option<Option<String>>,
    pub artist: Option<Option<String>>,
    pub label: Option<Option<String>>,
    pub disc_count: Option<Option<i64>>,
    pub isdn_region: Option<String>,
    pub isdn_class: Option<String>,
    pub isdn_type: Option<String>,
    pub isdn_rating_gender: Option<String>,
    pub isdn_rating_age: Option<String>,
    pub isdn_genre_code: Option<String>,
    pub isdn_genre_name: Option<String>,
    pub isdn_genre_user: Option<String>,
    pub isdn_c_code: Option<String>,
    pub isdn_author: Option<String>,
    pub isdn_shape: Option<String>,
    pub isdn_contents: Option<String>,
    pub isdn_barcode2: Option<String>,
    pub isdn_sample_image_url: Option<String>,
    pub isdn_useroption: Option<String>,
    pub isdn_external_links: Option<String>,
    pub reading_status: Option<String>,
    pub storage_location_id: Option<Option<i64>>,
    pub label_id: Option<Option<i64>>,
}

pub async fn update_book(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateBookRequest>,
) -> Result<StatusCode, ApiError> {
    let isbn = req
        .isbn
        .map(|s| s.trim().replace(['-', ' '], ""))
        .filter(|s| !s.is_empty());
    let isdn = req
        .isdn
        .map(|s| s.trim().replace(['-', ' '], ""))
        .filter(|s| !s.is_empty());
    if isbn.is_none() && isdn.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "ISBN or ISDN is required"})),
        ));
    }
    if req.title.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Title is required"})),
        ));
    }
    let existing_book = state.db.find_by_id(id).ok().flatten();

    // present/absent: Some(inner) = key present (inner None => clear), None = absent => preserve
    let series_id = match req.series_id {
        Some(o) => o,
        None => existing_book.as_ref().and_then(|b| b.series_id),
    };
    let series_number = match req.series_number {
        Some(o) => o,
        None => existing_book.as_ref().and_then(|b| b.series_number),
    };
    let media_type = match req.media_type {
        Some(o) => o,
        None => existing_book.as_ref().and_then(|b| b.media_type.clone()),
    };
    let catalog_number = match req.catalog_number {
        Some(o) => o,
        None => existing_book
            .as_ref()
            .and_then(|b| b.catalog_number.clone()),
    };
    let artist = match req.artist {
        Some(o) => o,
        None => existing_book.as_ref().and_then(|b| b.artist.clone()),
    };
    let label = match req.label {
        Some(o) => o,
        None => existing_book.as_ref().and_then(|b| b.label.clone()),
    };
    let disc_count = match req.disc_count {
        Some(o) => o,
        None => existing_book.as_ref().and_then(|b| b.disc_count),
    };
    let reading_status = req.reading_status.as_deref();
    let storage_location_id = match req.storage_location_id {
        Some(o) => o,
        None => existing_book.as_ref().and_then(|b| b.storage_location_id),
    };
    let label_id = match req.label_id {
        Some(o) => o,
        None => existing_book.as_ref().and_then(|b| b.label_id),
    };

    // grand_series_id: present => apply (Some(id) set / None clear), absent => leave as-is
    match req.grand_series_id {
        Some(gs_opt) => {
            if let Ok(Some(old_gs)) = state.db.get_book_grand_series(id) {
                let _ = state.db.remove_grand_series_item(old_gs.id, "book", id);
            }
            if let Some(gs_id) = gs_opt {
                if gs_id != 0 {
                    let _ = state.db.add_grand_series_item(gs_id, "book", id);
                }
            }
        }
        None => {}
    }

    state
        .db
        .update_book(
            id,
            isbn.as_deref(),
            isdn.as_deref(),
            None,
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
            series_number,
            req.isdn_region.as_deref(),
            req.isdn_class.as_deref(),
            req.isdn_type.as_deref(),
            req.isdn_rating_gender.as_deref(),
            req.isdn_rating_age.as_deref(),
            req.isdn_genre_code.as_deref(),
            req.isdn_genre_name.as_deref(),
            req.isdn_genre_user.as_deref(),
            req.isdn_c_code.as_deref(),
            req.isdn_author.as_deref(),
            req.isdn_shape.as_deref(),
            req.isdn_contents.as_deref(),
            req.isdn_barcode2.as_deref(),
            req.isdn_sample_image_url.as_deref(),
            req.isdn_useroption.as_deref(),
            req.isdn_external_links.as_deref(),
            media_type.as_deref(),
            catalog_number.as_deref(),
            artist.as_deref(),
            label.as_deref(),
            disc_count,
            reading_status,
            storage_location_id,
            label_id,
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
        .update_author(
            id,
            req.name.trim(),
            req.transcription.as_deref(),
            req.ndl_id.as_deref(),
        )
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
    let db = state.db.clone();
    let authors = tokio::task::spawn_blocking(move || db.list_authors())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(authors))
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
        .create_author(
            req.name.trim(),
            req.transcription.as_deref(),
            req.ndl_id.as_deref(),
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok((StatusCode::CREATED, Json(author)))
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

    let mut data: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "cover" {
            let ct = field.content_type().unwrap_or("image/jpeg").to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                })?
                .to_vec();
            let bytes = match chunk_info.as_ref() {
                Some(info) => match crate::api::upload_chunks::store_chunk(
                    state.uploads_dir.as_str(),
                    "cover",
                    info.clone(),
                    &bytes,
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
                    StoreResult::Complete { bytes, cleanup_dir } => {
                        let _ = std::fs::remove_dir_all(cleanup_dir);
                        bytes
                    }
                },
                None => bytes,
            };
            let cover_max = crate::api::upload_limit_bytes(
                &state,
                crate::api::KEY_UPLOAD_COVER_MB,
                crate::api::COVER_MAX_BYTES,
            );
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
        "image/gif" => "gif",
        _ => "jpg",
    };

    let hash = Sha3_256::digest(&bytes);
    let filename = format!(
        "{}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash),
        ext
    );
    let filepath = std::path::Path::new(state.images_dir.as_str()).join(&filename);

    std::fs::write(&filepath, &bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
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
            let old_path = std::path::Path::new(state.images_dir.as_str()).join(old);
            let _ = std::fs::remove_file(old_path);
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
        let old_path = std::path::Path::new(state.images_dir.as_str()).join(old);
        let _ = std::fs::remove_file(old_path);
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

    let mut data: Option<Vec<u8>> = None;
    let mut original_name: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
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
            let bytes = field
                .bytes()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                })?
                .to_vec();
            let bytes = match chunk_info.as_ref() {
                Some(info) => match crate::api::upload_chunks::store_chunk(
                    state.uploads_dir.as_str(),
                    "file",
                    info.clone(),
                    &bytes,
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
                    StoreResult::Complete { bytes, cleanup_dir } => {
                        let _ = std::fs::remove_dir_all(cleanup_dir);
                        bytes
                    }
                },
                None => bytes,
            };
            data = Some(bytes);
            original_name = Some(file_name);
        }
    }

    let bytes = data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No file uploaded"})),
        )
    })?;
    let original_name = original_name.unwrap_or_else(|| "upload.epub".to_string());

    let file_max = crate::api::upload_limit_bytes(
        &state,
        crate::api::KEY_UPLOAD_FILE_MB,
        crate::api::EPUB_MAX_BYTES,
    );
    let (saved_name, _ext) = crate::external::save_uploaded_file(
        &bytes,
        &original_name,
        state.epubs_dir.as_str(),
        file_max,
    )
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
