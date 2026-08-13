use dantalian::application::error::AppError;
use dantalian::ports::object_storage::{
    MULTIPART_PART_SIZE, MultipartPart, MultipartUploadStorage, ObjectKind, object_key,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use worker::{D1Type, Date, Request, Response, Result, RouteContext};

use crate::error::{bad_request, error_response, parse_json};
use crate::wasabi::{WasabiConfig, WasabiStorage};

const MAX_MULTIPART_SIZE: u64 = 5 * 1024 * 1024 * 1024 * 1024;
const OWNER_SCOPE: &str = "api-token";
const OBJECT_KIND: &str = "epub";

#[derive(Debug, Deserialize)]
pub struct MultipartInitRequest {
    pub expected_size: u64,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
struct MultipartInitResponse {
    id: String,
    object_key: String,
    content_type: String,
    part_size: u64,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct PartSignResponse {
    upload_url: String,
    part_number: u32,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct MultipartCompleteRequest {
    pub parts: Vec<MultipartPartRequest>,
}

#[derive(Debug, Deserialize)]
pub struct MultipartPartRequest {
    pub part_number: u32,
    pub etag: String,
}

#[derive(Debug, Serialize)]
struct MultipartCompleteResponse {
    id: String,
    object_key: String,
    expected_size: u64,
    content_type: String,
    status: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
struct MultipartSession {
    id: String,
    provider_upload_id: String,
    object_key: String,
    object_kind: String,
    expected_size: String,
    content_type: String,
    status: String,
    owner: String,
    created_at: String,
}

pub async fn init(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<MultipartInitRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let content_type = match validate_init(&request) {
        Ok(content_type) => content_type,
        Err(error) => return error_response(error),
    };
    let config = match WasabiConfig::from_env(&ctx.env) {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let id = Uuid::new_v4().simple().to_string();
    let key = match object_key(config.prefix.as_deref(), ObjectKind::Epub, &id, "epub") {
        Ok(key) => key,
        Err(error) => return error_response(error),
    };
    let storage = WasabiStorage::new(config);
    let provider_upload_id = match storage.create_multipart_upload(&key, &content_type).await {
        Ok(upload_id) => upload_id,
        Err(error) => return error_response(error),
    };
    let session = MultipartSession {
        id: id.clone(),
        provider_upload_id,
        object_key: key.clone(),
        object_kind: OBJECT_KIND.to_string(),
        expected_size: request.expected_size.to_string(),
        content_type: content_type.clone(),
        status: "pending".to_string(),
        owner: OWNER_SCOPE.to_string(),
        created_at: Date::now().as_millis().to_string(),
    };
    let db = match ctx.d1("DB") {
        Ok(db) => db,
        Err(error) => {
            let _ = storage
                .abort_multipart_upload(&key, &session.provider_upload_id)
                .await;
            return error_response(AppError::Database(error.to_string()));
        }
    };
    if let Err(error) = insert_session(&db, &session).await {
        let _ = storage
            .abort_multipart_upload(&key, &session.provider_upload_id)
            .await;
        return error_response(error);
    }
    Response::from_json(&MultipartInitResponse {
        id,
        object_key: key,
        content_type,
        part_size: MULTIPART_PART_SIZE,
        status: "pending",
    })
}

pub async fn sign_part(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let session_id = match session_id(&ctx) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let part_number = match part_number(&ctx) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let session = match load_session(&db, &session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => return error_response(AppError::NotFound),
        Err(error) => return error_response(error),
    };
    if session.owner != OWNER_SCOPE {
        return error_response(AppError::NotFound);
    }
    if !matches!(session.status.as_str(), "pending" | "uploading") {
        return error_response(AppError::Conflict(
            "multipart session is no longer uploadable".to_string(),
        ));
    }
    let config = match WasabiConfig::from_env(&ctx.env) {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let storage = WasabiStorage::new(config);
    let upload_url = match storage.presigned_upload_part_url(
        &session.object_key,
        &session.provider_upload_id,
        part_number,
    ) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    if session.status == "pending" {
        if let Err(error) = update_status(&db, &session.id, "uploading").await {
            return error_response(error);
        }
    }
    Response::from_json(&PartSignResponse {
        upload_url,
        part_number,
        expires_in: crate::wasabi::UPLOAD_URL_TTL_SECONDS,
    })
}

pub async fn complete(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<MultipartCompleteRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let session_id = match session_id(&ctx) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let session = match load_session(&db, &session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => return error_response(AppError::NotFound),
        Err(error) => return error_response(error),
    };
    if session.owner != OWNER_SCOPE {
        return error_response(AppError::NotFound);
    }
    if !matches!(session.status.as_str(), "pending" | "uploading") {
        return error_response(AppError::Conflict(
            "multipart session is no longer completable".to_string(),
        ));
    }
    let parts = match validate_parts(&request.parts) {
        Ok(parts) => parts,
        Err(error) => return error_response(error),
    };
    let config = match WasabiConfig::from_env(&ctx.env) {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let storage = WasabiStorage::new(config);
    let metadata = match storage
        .complete_multipart_upload(&session.object_key, &session.provider_upload_id, &parts)
        .await
    {
        Ok(metadata) => metadata,
        Err(error) => return error_response(error),
    };
    let expected_size = match session.expected_size.parse::<u64>() {
        Ok(value) => value,
        Err(_) => return error_response(AppError::Database("invalid multipart size".to_string())),
    };
    if metadata.content_length != expected_size
        || metadata
            .content_type
            .as_deref()
            .is_some_and(|value| !value.eq_ignore_ascii_case(&session.content_type))
    {
        let _ = storage.delete_object(&session.object_key).await;
        let _ = update_status(&db, &session.id, "failed").await;
        return error_response(AppError::Conflict(
            "completed object metadata does not match multipart session".to_string(),
        ));
    }
    if let Err(error) = update_status(&db, &session.id, "complete").await {
        return error_response(error);
    }
    Response::from_json(&MultipartCompleteResponse {
        id: session.id,
        object_key: session.object_key,
        expected_size,
        content_type: session.content_type,
        status: "complete",
    })
}

pub async fn abort(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let session_id = match session_id(&ctx) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let session = match load_session(&db, &session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => return error_response(AppError::NotFound),
        Err(error) => return error_response(error),
    };
    if session.owner != OWNER_SCOPE {
        return error_response(AppError::NotFound);
    }
    if matches!(session.status.as_str(), "complete" | "aborted") {
        return error_response(AppError::Conflict(
            "multipart session is no longer abortable".to_string(),
        ));
    }
    let config = match WasabiConfig::from_env(&ctx.env) {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let storage = WasabiStorage::new(config);
    if let Err(error) = storage
        .abort_multipart_upload(&session.object_key, &session.provider_upload_id)
        .await
        && !matches!(error, AppError::NotFound)
    {
        return error_response(error);
    }
    if let Err(error) = update_status(&db, &session.id, "aborted").await {
        return error_response(error);
    }
    Ok(Response::empty()?.with_status(204))
}

fn validate_init(request: &MultipartInitRequest) -> Result<String, AppError> {
    if request.expected_size == 0 || request.expected_size > MAX_MULTIPART_SIZE {
        return Err(AppError::Validation(
            "expected_size must be between 1 byte and 5 TiB".to_string(),
        ));
    }
    let content_type = request.content_type.trim().to_ascii_lowercase();
    if content_type != "application/epub+zip" && content_type != "application/octet-stream" {
        return Err(AppError::Validation(
            "multipart uploads currently support EPUB content only".to_string(),
        ));
    }
    Ok(content_type)
}

fn validate_parts(parts: &[MultipartPartRequest]) -> Result<Vec<MultipartPart>, AppError> {
    if parts.is_empty() {
        return Err(AppError::Validation(
            "multipart completion requires parts".to_string(),
        ));
    }
    let mut previous = 0;
    let mut validated = Vec::with_capacity(parts.len());
    for part in parts {
        if !(1..=10_000).contains(&part.part_number) || part.part_number != previous + 1 {
            return Err(AppError::Validation(
                "multipart parts must be ordered consecutively from part 1".to_string(),
            ));
        }
        if part.etag.is_empty()
            || part.etag.len() > 1024
            || part
                .etag
                .chars()
                .any(|value| value.is_control() || matches!(value, '<' | '>' | '&'))
        {
            return Err(AppError::Validation("invalid multipart ETag".to_string()));
        }
        validated.push(MultipartPart {
            part_number: part.part_number,
            etag: part.etag.clone(),
        });
        previous = part.part_number;
    }
    Ok(validated)
}

fn session_id(ctx: &RouteContext<()>) -> std::result::Result<String, Response> {
    let value = ctx.param("id").map(String::as_str).unwrap_or_default();
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad_request("invalid multipart session id"));
    }
    Ok(value.to_string())
}

fn part_number(ctx: &RouteContext<()>) -> std::result::Result<u32, Response> {
    let value = ctx
        .param("part_number")
        .map(String::as_str)
        .unwrap_or_default();
    let value = value
        .parse::<u32>()
        .ok()
        .filter(|value| (1..=10_000).contains(value))
        .ok_or_else(|| bad_request("invalid multipart part number"))?;
    Ok(value)
}

async fn insert_session(
    db: &worker::D1Database,
    session: &MultipartSession,
) -> std::result::Result<(), AppError> {
    db.prepare(
        "INSERT INTO multipart_uploads
         (id, provider_upload_id, object_key, object_kind, expected_size, content_type, status, owner, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind_refs([
        &D1Type::Text(&session.id),
        &D1Type::Text(&session.provider_upload_id),
        &D1Type::Text(&session.object_key),
        &D1Type::Text(&session.object_kind),
        &D1Type::Text(&session.expected_size),
        &D1Type::Text(&session.content_type),
        &D1Type::Text(&session.status),
        &D1Type::Text(&session.owner),
        &D1Type::Text(&session.created_at),
    ])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    Ok(())
}

async fn load_session(
    db: &worker::D1Database,
    id: &str,
) -> std::result::Result<Option<MultipartSession>, AppError> {
    db.prepare(
        "SELECT id, provider_upload_id, object_key, object_kind, expected_size, content_type, status, owner, created_at
         FROM multipart_uploads WHERE id = ?",
    )
    .bind_refs(&D1Type::Text(id))
    .map_err(db_error)?
    .first::<MultipartSession>(None)
    .await
    .map_err(db_error)
}

async fn update_status(
    db: &worker::D1Database,
    id: &str,
    status: &str,
) -> std::result::Result<(), AppError> {
    db.prepare("UPDATE multipart_uploads SET status = ? WHERE id = ?")
        .bind_refs([&D1Type::Text(status), &D1Type::Text(id)])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(())
}

fn db_error(error: impl std::fmt::Display) -> AppError {
    AppError::Database(error.to_string())
}
