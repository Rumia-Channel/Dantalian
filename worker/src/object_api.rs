use sha2::{Digest, Sha256};
use worker::{D1Type, Date, FormEntry, Request, Response, Result, RouteContext};

use dantalian::{
    application::error::AppError,
    ports::object_storage::{AudioCodec, ObjectKind, WORKER_DIRECT_UPLOAD_MAX_BYTES, object_key},
};

use crate::{
    error::{bad_request, error_response, parse_id},
    wasabi::{WasabiConfig, WasabiStorage},
};

const COVER_MAX_BYTES: usize = 10 * 1024 * 1024;

fn db_error(error: worker::Error) -> worker::Error {
    error
}

fn id_type(id: i64, label: &str) -> std::result::Result<D1Type<'static>, AppError> {
    let id = i32::try_from(id).map_err(|_| AppError::Validation(format!("invalid {label}")))?;
    if id <= 0 {
        return Err(AppError::Validation(format!("invalid {label}")));
    }
    Ok(D1Type::Integer(id))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn extension(name: &str, fallback: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, value)| value.to_ascii_lowercase())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 12
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn split_object_name(name: &str) -> std::result::Result<(&str, &str), AppError> {
    let (object_id, extension) = name
        .rsplit_once('.')
        .ok_or_else(|| AppError::Validation("invalid object name".to_string()))?;
    if object_id.is_empty() || extension.is_empty() {
        return Err(AppError::Validation("invalid object name".to_string()));
    }
    Ok((object_id, extension))
}

fn key_for_name(
    config: &WasabiConfig,
    kind: ObjectKind,
    name: &str,
) -> std::result::Result<String, AppError> {
    let (object_id, extension) = split_object_name(name)?;
    object_key(config.prefix.as_deref(), kind, object_id, extension)
}

fn max_direct_size(kind: ObjectKind) -> usize {
    match kind {
        ObjectKind::CoverImage => COVER_MAX_BYTES,
        ObjectKind::Epub | ObjectKind::OriginalAudio | ObjectKind::EncodedAudio { .. } => {
            usize::try_from(WORKER_DIRECT_UPLOAD_MAX_BYTES)
                .expect("Worker upload limit fits in usize")
        }
    }
}

pub async fn stream(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let file_hash = match ctx.param("file_hash") {
        Some(value) if !value.is_empty() => value.as_str(),
        _ => return Ok(bad_request("missing file hash")),
    };
    let query = req.url()?.query_pairs().into_owned().collect::<Vec<_>>();
    let format = query
        .iter()
        .find(|(key, _)| key == "format")
        .map(|(_, value)| value.as_str());
    let format = match format {
        Some("opus") => Some(AudioCodec::Opus),
        Some("aac") => Some(AudioCodec::Aac),
        Some(_) => return Ok(bad_request("format must be opus or aac")),
        None => None,
    };
    let requested_extension = query
        .iter()
        .find(|(key, _)| key == "ext")
        .map(|(_, value)| value.to_string());
    let extension = if format.is_some() {
        "bin".to_string()
    } else if let Some(extension) = requested_extension {
        extension
    } else {
        let db = ctx.d1("DB")?;
        let hash = D1Type::Text(file_hash);
        db.prepare("SELECT file_name FROM tracks WHERE file_hash = ? LIMIT 1")
            .bind_refs(&hash)
            .map_err(db_error)?
            .first::<serde_json::Value>(None)
            .await
            .map_err(db_error)?
            .and_then(|row| {
                row.get("file_name")
                    .and_then(|value| value.as_str())
                    .map(|name| extension(name, "bin"))
            })
            .unwrap_or_else(|| "bin".to_string())
    };
    let kind = format
        .map(|codec| ObjectKind::EncodedAudio { codec })
        .unwrap_or(ObjectKind::OriginalAudio);
    let config = match WasabiConfig::from_env(&ctx.env).await {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let key = match object_key(config.prefix.as_deref(), kind, file_hash, &extension) {
        Ok(key) => key,
        Err(error) => return error_response(error),
    };
    let url = WasabiStorage::new(config)
        .presigned_get_url(&key)
        .map_err(|error| worker::Error::from(error.to_string()))?;
    Response::redirect(
        worker::Url::parse(&url).map_err(|error| worker::Error::RustError(error.to_string()))?,
    )
}

async fn redirect_file(ctx: &RouteContext<()>, name: &str, kind: ObjectKind) -> Result<Response> {
    let config = match WasabiConfig::from_env(&ctx.env).await {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let key = match key_for_name(&config, kind, name) {
        Ok(key) => key,
        Err(error) => return error_response(error),
    };
    let url = WasabiStorage::new(config)
        .presigned_get_url(&key)
        .map_err(|error| worker::Error::from(error.to_string()))?;
    Response::redirect(
        worker::Url::parse(&url).map_err(|error| worker::Error::RustError(error.to_string()))?,
    )
}

pub async fn image(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let Some(name) = ctx.param("file_hash") else {
        return Ok(bad_request("missing image name"));
    };
    redirect_file(&ctx, name, ObjectKind::CoverImage).await
}

pub async fn epub(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let Some(name) = ctx.param("file_hash") else {
        return Ok(bad_request("missing epub name"));
    };
    redirect_file(&ctx, name, ObjectKind::Epub).await
}

async fn ensure_target(
    ctx: &RouteContext<()>,
    entity_kind: &str,
    entity_id: i64,
    track_id: Option<i64>,
) -> std::result::Result<(D1Type<'static>, Option<D1Type<'static>>), AppError> {
    let db = ctx
        .d1("DB")
        .map_err(|error| AppError::Database(error.to_string()))?;
    let entity_id = id_type(entity_id, "entity id")?;
    match (entity_kind, track_id) {
        ("book-audio" | "cd-audio", Some(track_id)) => {
            let track_id = id_type(track_id, "track id")?;
            let column = if entity_kind == "book-audio" {
                "book_id"
            } else {
                "cd_id"
            };
            let sql = format!("SELECT id FROM tracks WHERE id = ? AND {column} = ?");
            let row = db
                .prepare(&sql)
                .bind_refs([&track_id, &entity_id])
                .map_err(|error| AppError::Database(error.to_string()))?
                .first::<serde_json::Value>(None)
                .await
                .map_err(|error| AppError::Database(error.to_string()))?;
            if row.is_none() {
                return Err(AppError::NotFound);
            }
            Ok((entity_id, Some(track_id)))
        }
        ("book-cover" | "book-epub", None) => {
            let row = db
                .prepare("SELECT id FROM books WHERE id = ?")
                .bind_refs(&entity_id)
                .map_err(|error| AppError::Database(error.to_string()))?
                .first::<serde_json::Value>(None)
                .await
                .map_err(|error| AppError::Database(error.to_string()))?;
            if row.is_none() {
                return Err(AppError::NotFound);
            }
            Ok((entity_id, None))
        }
        ("cd-cover", None) => {
            let row = db
                .prepare("SELECT id FROM cds WHERE id = ?")
                .bind_refs(&entity_id)
                .map_err(|error| AppError::Database(error.to_string()))?
                .first::<serde_json::Value>(None)
                .await
                .map_err(|error| AppError::Database(error.to_string()))?;
            if row.is_none() {
                return Err(AppError::NotFound);
            }
            Ok((entity_id, None))
        }
        _ => Err(AppError::Validation(
            "unsupported object upload target".to_string(),
        )),
    }
}

async fn upload(
    mut req: Request,
    ctx: RouteContext<()>,
    kind: ObjectKind,
    entity_kind: &str,
    field: &str,
) -> Result<Response> {
    let entity_id = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let track_id = if matches!(kind, ObjectKind::OriginalAudio) {
        Some(match parse_id(&ctx, "track_id") {
            Ok(value) => value,
            Err(response) => return Ok(response),
        })
    } else {
        None
    };
    let (entity_id, track_id) = match ensure_target(&ctx, entity_kind, entity_id, track_id).await {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let form = match req.form_data().await {
        Ok(form) => form,
        Err(error) => return Ok(bad_request(format!("invalid multipart form: {error}"))),
    };
    let file = match form.get(field) {
        Some(FormEntry::File(file)) => file,
        _ => return Ok(bad_request(format!("missing multipart field: {field}"))),
    };
    if file.size() > max_direct_size(kind) {
        return Response::from_json(&serde_json::json!({
            "error": "file exceeds the Worker direct upload limit",
            "code": "presigned_multipart_required",
            "max_bytes": max_direct_size(kind),
        }))
        .map(|response| response.with_status(413));
    }
    let bytes = match file.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => return Ok(bad_request(format!("invalid uploaded file: {error}"))),
    };
    let file_hash = hash_bytes(&bytes);
    let fallback = match kind {
        ObjectKind::CoverImage => "jpg",
        ObjectKind::Epub => "epub",
        ObjectKind::OriginalAudio | ObjectKind::EncodedAudio { .. } => "bin",
    };
    let extension = extension(&file.name(), fallback);
    let file_name = format!("{file_hash}.{extension}");
    let config = match WasabiConfig::from_env(&ctx.env).await {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let key = match object_key(config.prefix.as_deref(), kind, &file_hash, &extension) {
        Ok(key) => key,
        Err(error) => return error_response(error),
    };
    let original_name = file.name();
    let content_type = if file.type_().is_empty() {
        "application/octet-stream".to_string()
    } else {
        file.type_()
    };
    let storage = WasabiStorage::new(config.clone());
    storage
        .put_object(&key, &content_type, &bytes)
        .await
        .map_err(|error| worker::Error::from(error.to_string()))?;
    let db = ctx.d1("DB")?;
    match (entity_kind, track_id) {
        ("book-cover", None) => {
            db.prepare(
                "UPDATE books SET cover_url = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind_refs([&D1Type::Text(&file_name), &entity_id])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
        }
        ("book-epub", None) => {
            db.prepare("UPDATE books SET epub_file_hash = ?, epub_file_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind_refs([
                    &D1Type::Text(&file_name),
                    &D1Type::Text(&original_name),
                    &entity_id,
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
        }
        ("cd-cover", None) => {
            db.prepare("UPDATE cds SET cover_url = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind_refs([&D1Type::Text(&file_name), &entity_id])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
        }
        ("book-audio" | "cd-audio", Some(track_id)) => {
            db.prepare("UPDATE tracks SET file_hash = ?, file_name = ? WHERE id = ?")
                .bind_refs([
                    &D1Type::Text(&file_hash),
                    &D1Type::Text(&original_name),
                    &track_id,
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
        }
        _ => return Ok(bad_request("unsupported object upload target")),
    }
    let storage = WasabiStorage::new(config.clone());
    let object_kind = match kind {
        ObjectKind::CoverImage => "cover",
        ObjectKind::Epub => "epub",
        ObjectKind::OriginalAudio | ObjectKind::EncodedAudio { .. } => "audio",
    };
    let created_at = Date::now().as_millis().to_string();
    db.prepare(
        "INSERT OR REPLACE INTO object_uploads
         (object_key, object_kind, entity_id, content_type, extension, expected_size, original_name, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'complete', ?)",
    )
    .bind_refs([
        &D1Type::Text(&key),
        &D1Type::Text(object_kind),
        &entity_id,
        &D1Type::Text(&content_type),
        &D1Type::Text(&extension),
        &D1Type::Integer(i32::try_from(bytes.len()).unwrap_or(i32::MAX)),
        &D1Type::Text(&original_name),
        &D1Type::Text(&created_at),
    ])
    .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    if matches!(kind, ObjectKind::OriginalAudio) {
        if let Err(error) = crate::audio_job_api::enqueue_data_saver_jobs(&ctx.env).await {
            worker::console_error!("data saver job scheduling deferred after upload: {error}");
        }
    }
    let download_url = storage
        .presigned_get_url(&key)
        .map_err(|error| worker::Error::from(error.to_string()))?;
    Response::from_json(&serde_json::json!({
        "file_hash": file_name,
        "object_key": key,
        "download_url": download_url,
    }))
}

pub async fn book_cover(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    upload(req, ctx, ObjectKind::CoverImage, "book-cover", "cover").await
}

pub async fn book_epub(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    upload(req, ctx, ObjectKind::Epub, "book-epub", "file").await
}

pub async fn cd_cover(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    upload(req, ctx, ObjectKind::CoverImage, "cd-cover", "cover").await
}

pub async fn book_audio(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    upload(req, ctx, ObjectKind::OriginalAudio, "book-audio", "audio").await
}

pub async fn cd_audio(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    upload(req, ctx, ObjectKind::OriginalAudio, "cd-audio", "audio").await
}

async fn delete_named_object(
    ctx: &RouteContext<()>,
    table: &str,
    column: &str,
    secondary_column: Option<&str>,
    kind: ObjectKind,
) -> Result<Response> {
    let raw = match parse_id(ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, "entity id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let select = match secondary_column {
        Some(secondary) => format!("SELECT {column}, {secondary} FROM {table} WHERE id = ?"),
        None => format!("SELECT {column} FROM {table} WHERE id = ?"),
    };
    let row = db
        .prepare(&select)
        .bind_refs(&id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    let Some(row) = row else {
        return error_response(AppError::NotFound);
    };
    let value = row
        .get(column)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let config = match WasabiConfig::from_env(&ctx.env).await {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    if let Some(value) = value {
        let key = match key_for_name(&config, kind, &value) {
            Ok(key) => key,
            Err(error) => return error_response(error),
        };
        if let Err(error) = WasabiStorage::new(config.clone()).delete_object(&key).await {
            if !matches!(error, AppError::NotFound) {
                return error_response(error);
            }
        }
    }
    let update = match secondary_column {
        Some(secondary) => {
            format!(
                "UPDATE {table} SET {column} = NULL, {secondary} = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
        }
        None => format!(
            "UPDATE {table} SET {column} = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        ),
    };
    db.prepare(&update)
        .bind_refs(&id)
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}

pub async fn delete_book_cover(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    delete_named_object(&ctx, "books", "cover_url", None, ObjectKind::CoverImage).await
}

pub async fn delete_book_epub(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    delete_named_object(
        &ctx,
        "books",
        "epub_file_hash",
        Some("epub_file_name"),
        ObjectKind::Epub,
    )
    .await
}

pub async fn delete_cd_cover(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    delete_named_object(&ctx, "cds", "cover_url", None, ObjectKind::CoverImage).await
}

async fn delete_track_audio(ctx: &RouteContext<()>, parent_column: &str) -> Result<Response> {
    let parent = match parse_id(ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let track = match parse_id(ctx, "track_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let parent = match id_type(parent, "parent id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let track = match id_type(track, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let row = db
        .prepare(&format!(
            "SELECT file_hash, file_name FROM tracks WHERE id = ? AND {parent_column} = ?"
        ))
        .bind_refs([&track, &parent])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    let Some(row) = row else {
        return error_response(AppError::NotFound);
    };
    let file_hash = row
        .get("file_hash")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let file_name = row
        .get("file_name")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    if let (Some(file_hash), Some(file_name)) = (file_hash, file_name) {
        let config = match WasabiConfig::from_env(&ctx.env).await {
            Ok(config) => config,
            Err(error) => return error_response(AppError::Storage(error.to_string())),
        };
        let name = format!("{file_hash}.{}", extension(&file_name, "bin"));
        let key = match key_for_name(&config, ObjectKind::OriginalAudio, &name) {
            Ok(key) => key,
            Err(error) => return error_response(error),
        };
        if let Err(error) = WasabiStorage::new(config).delete_object(&key).await {
            if !matches!(error, AppError::NotFound) {
                return error_response(error);
            }
        }
    }
    db.prepare("UPDATE tracks SET file_hash = NULL, file_name = NULL WHERE id = ?")
        .bind_refs(&track)
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}

pub async fn delete_book_audio(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    delete_track_audio(&ctx, "book_id").await
}

pub async fn delete_cd_audio(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    delete_track_audio(&ctx, "cd_id").await
}
