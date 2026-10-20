//! Presigned multipart finalize for book EPUB files.
//!
//! Large EPUBs are uploaded straight from the browser to Wasabi in parts, so the
//! Worker never receives the file body. The finalize step verifies the completed
//! multipart session against the object metadata and binds it to the book row.
//! Audio tracks use the same contract through `track_api::finalize_*`.

use dantalian::application::error::AppError;
use dantalian::ports::object_storage::validate_object_key;
use serde::Deserialize;
use worker::{D1Type, Date, Request, Response, Result, RouteContext};

use crate::error::{bad_request, error_response, parse_id, parse_json};
use crate::multipart_api::EPUB_OBJECT_KIND;
use crate::wasabi::{WasabiConfig, WasabiStorage};

/// Request body for the presigned EPUB finalize step. The browser uploads the
/// parts directly to Wasabi, then hands the completed session back so the book
/// row can reference the object.
#[derive(Debug, Deserialize)]
pub struct BookEpubFinalizeRequest {
    pub session_id: String,
    /// Original client file name, recorded on the book row and in `object_uploads`.
    #[serde(default)]
    pub name: Option<String>,
}

/// Object key and session details of a completed presigned multipart upload.
struct CompletedEpubUpload {
    object_key: String,
    object_kind: String,
    /// Canonical `<hash>.<extension>` name the book row references.
    object_name: String,
    extension: String,
    content_type: String,
    expected_size: i64,
    original_name: Option<String>,
}

/// Binds a completed presigned multipart EPUB object to a book.
///
/// The Worker never sees the EPUB bytes on this path: it verifies the session
/// metadata and the object size in Wasabi, records the object, and updates the
/// book row.
pub async fn finalize(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw_book = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let book = match book_id_type(raw_book) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match parse_json::<BookEpubFinalizeRequest>(&mut req).await {
        Ok(body) => body,
        Err(response) => return Ok(response),
    };
    if !valid_session_id(&body.session_id) {
        return Ok(bad_request("invalid multipart session id"));
    }
    let db = ctx.d1("DB")?;
    let book_exists = db
        .prepare("SELECT id FROM books WHERE id = ?")
        .bind_refs(&book)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .is_some();
    if !book_exists {
        return error_response(AppError::NotFound);
    }
    let session = match load_completed_epub_session(&ctx, &body.session_id, body.name).await {
        Ok(session) => session,
        Err(error) => return error_response(error),
    };
    let config = match WasabiConfig::from_env(&ctx.env).await {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let storage = WasabiStorage::new(config);
    let stored = match storage.head_object(&session.object_key).await {
        Ok(metadata) => metadata.content_length,
        Err(error) => return error_response(error),
    };
    if stored != Some(u64::try_from(session.expected_size).unwrap_or(u64::MAX)) {
        return error_response(AppError::Conflict(
            "uploaded EPUB object size does not match the multipart session".to_string(),
        ));
    }
    let created_at = Date::now().as_millis().to_string();
    // Claim the session before touching the book row: only the first finalize
    // can move complete -> finalized, so one object cannot be attached twice.
    let claimed = db
        .prepare(
            "UPDATE multipart_uploads SET status = 'finalized' WHERE id = ? AND status = 'complete'",
        )
        .bind_refs(&D1Type::Text(&body.session_id))
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    if claimed
        .meta()
        .map_err(db_error)?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        == 0
    {
        return error_response(AppError::Conflict(
            "multipart session has already been attached to a book".to_string(),
        ));
    }
    let display_name = session
        .original_name
        .as_deref()
        .unwrap_or(&session.object_name);
    db.prepare(
        "UPDATE books SET epub_file_hash = ?, epub_file_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind_refs([
        &D1Type::Text(&session.object_name),
        &D1Type::Text(display_name),
        &book,
    ])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    db.prepare(
        "INSERT OR REPLACE INTO object_uploads
         (object_key, object_kind, entity_id, content_type, extension, expected_size, original_name, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'complete', ?)",
    )
    .bind_refs([
        &D1Type::Text(&session.object_key),
        &D1Type::Text(&session.object_kind),
        &book,
        &D1Type::Text(&session.content_type),
        &D1Type::Text(&session.extension),
        &D1Type::Integer(i32::try_from(session.expected_size).unwrap_or(i32::MAX)),
        &session
            .original_name
            .as_deref()
            .map(D1Type::Text)
            .unwrap_or(D1Type::Null),
        &D1Type::Text(&created_at),
    ])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    Response::from_json(&serde_json::json!({
        "file_hash": session.object_name,
        "file_name": display_name,
        "object_key": session.object_key,
        "object_kind": session.object_kind,
        "size": session.expected_size,
    }))
}

/// Loads a multipart session that has already been completed against Wasabi and
/// proves it is an EPUB object of the canonical `epubs/<hash>.<extension>` shape,
/// so the book row can be derived from the session alone.
async fn load_completed_epub_session(
    ctx: &RouteContext<()>,
    session_id: &str,
    original_name: Option<String>,
) -> std::result::Result<CompletedEpubUpload, AppError> {
    let db = ctx
        .d1("DB")
        .map_err(|error| AppError::Database(error.to_string()))?;
    let row = db
        .prepare(
            "SELECT object_key, object_kind, expected_size, content_type, status
             FROM multipart_uploads WHERE id = ?",
        )
        .bind_refs(&D1Type::Text(session_id))
        .map_err(|error| AppError::Database(error.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|error| AppError::Database(error.to_string()))?;
    let Some(row) = row else {
        return Err(AppError::NotFound);
    };
    if row.get("status").and_then(|value| value.as_str()) != Some("complete") {
        return Err(AppError::Conflict(
            "multipart session is not complete".to_string(),
        ));
    }
    if row.get("object_kind").and_then(|value| value.as_str()) != Some(EPUB_OBJECT_KIND) {
        return Err(AppError::Validation(
            "multipart session is not an EPUB upload".to_string(),
        ));
    }
    let object_key = row
        .get("object_key")
        .and_then(|value| value.as_str())
        .ok_or_else(|| AppError::Database("multipart session has no object key".to_string()))?
        .to_string();
    let content_type = row
        .get("content_type")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Database("multipart session has no content type".to_string()))?
        .to_string();
    let expected_size = row
        .get("expected_size")
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| AppError::Database("multipart session has no size".to_string()))?;
    let (stem, extension) = epub_key_parts(&object_key)?;
    Ok(CompletedEpubUpload {
        object_key,
        object_kind: EPUB_OBJECT_KIND.to_string(),
        object_name: format!("{stem}.{extension}"),
        extension,
        content_type,
        expected_size,
        original_name: original_name.and_then(sanitize_original_name),
    })
}

/// `epubs/<hash>.<extension>` (optionally behind a deployment prefix) is the only
/// key shape a book can reference, so anything else is rejected before it can be
/// written into the media tables.
fn epub_key_parts(object_key: &str) -> std::result::Result<(String, String), AppError> {
    validate_object_key(object_key)?;
    let components = object_key.split('/').collect::<Vec<_>>();
    if components.len() < 2 || components[components.len() - 2] != "epubs" {
        return Err(AppError::Validation(
            "epub objects must live under the epubs/ prefix".to_string(),
        ));
    }
    let name = components[components.len() - 1];
    let (stem, extension) = name
        .rsplit_once('.')
        .ok_or_else(|| AppError::Validation("invalid epub object name".to_string()))?;
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(AppError::Validation("invalid epub object name".to_string()));
    }
    let extension = extension.to_ascii_lowercase();
    if extension != "epub" {
        return Err(AppError::Validation(
            "epub objects must end with .epub".to_string(),
        ));
    }
    Ok((stem.to_string(), extension))
}

fn sanitize_original_name(name: String) -> Option<String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 512 || name.chars().any(char::is_control) {
        return None;
    }
    Some(name)
}

fn valid_session_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn book_id_type(id: i64) -> std::result::Result<D1Type<'static>, AppError> {
    let value =
        i32::try_from(id).map_err(|_| AppError::Validation("invalid book id".to_string()))?;
    if value <= 0 {
        return Err(AppError::Validation("invalid book id".to_string()));
    }
    Ok(D1Type::Integer(value))
}

fn db_error(error: worker::Error) -> worker::Error {
    error
}

#[cfg(test)]
mod tests {
    use super::{epub_key_parts, sanitize_original_name, valid_session_id};

    #[test]
    fn epub_key_parts_accepts_only_canonical_epub_objects() {
        assert_eq!(
            epub_key_parts("epubs/0123456789abcdef0123456789abcdef.epub").expect("canonical key"),
            (
                "0123456789abcdef0123456789abcdef".to_string(),
                "epub".to_string()
            )
        );
        assert_eq!(
            epub_key_parts("dantalian/production/epubs/book.epub").expect("prefixed key"),
            ("book".to_string(), "epub".to_string())
        );
        assert_eq!(
            epub_key_parts("epubs/book.EPUB").expect("upper-case extension"),
            ("book".to_string(), "epub".to_string())
        );
        assert!(epub_key_parts("audio/abc.mp3").is_err());
        assert!(epub_key_parts("epubs/book.pdf").is_err());
        assert!(epub_key_parts("epubs/book").is_err());
        assert!(epub_key_parts("epubs/..epub").is_err());
        assert!(epub_key_parts("epubs/../secret.epub").is_err());
    }

    #[test]
    fn session_ids_and_original_names_are_bounded() {
        assert!(valid_session_id("0123456789abcdef0123456789abcdef"));
        assert!(!valid_session_id("0123"));
        assert!(!valid_session_id("0123456789abcdef0123456789abcdeZ"));

        assert_eq!(
            sanitize_original_name("  book.epub  ".to_string()),
            Some("book.epub".to_string())
        );
        assert_eq!(sanitize_original_name("   ".to_string()), None);
        assert_eq!(sanitize_original_name("bad\nname.epub".to_string()), None);
        assert_eq!(sanitize_original_name("x".repeat(513)), None);
    }
}
