use dantalian::{
    application::error::AppError,
    ports::object_storage::{ObjectKind, ObjectStorage, object_key},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use worker::{D1Type, Date, Request, Response, Result, RouteContext};

use crate::{
    error::{bad_request, error_response, parse_json},
    wasabi::{WasabiConfig, WasabiStorage},
};

const MAX_COVER_BYTES: i64 = 10 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct CoverInitRequest {
    pub content_type: String,
    pub extension: String,
    pub size: i64,
    pub book_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CoverInitResponse {
    object_key: String,
    upload_url: String,
    expires_in: u64,
    content_type: String,
}

#[derive(Debug, Deserialize)]
pub struct CoverCompleteRequest {
    pub object_key: String,
}

#[derive(Debug, Deserialize)]
struct CoverObjectRecord {
    object_key: String,
    book_id: Option<i64>,
    content_type: String,
    extension: String,
    expected_size: i64,
    status: String,
}

#[derive(Debug, Serialize)]
struct CoverCompleteResponse {
    object_key: String,
    book_id: Option<i64>,
    download_url: String,
}

pub async fn init(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CoverInitRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let (content_type, extension) = match validate_cover_metadata(&request) {
        Ok(metadata) => metadata,
        Err(error) => return error_response(error),
    };
    let config = match WasabiConfig::from_env(&ctx.env) {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(storage_message(error))),
    };
    let storage = WasabiStorage::new(config.clone());
    let object_id = Uuid::new_v4().simple().to_string();
    let key = match object_key(
        config.prefix.as_deref(),
        ObjectKind::CoverImage,
        &object_id,
        &extension,
    ) {
        Ok(key) => key,
        Err(error) => return error_response(error),
    };
    let upload_url = match storage.presigned_put_url(&key, &content_type) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };

    let db = match ctx.d1("DB") {
        Ok(db) => db,
        Err(error) => return error_response(AppError::Database(error.to_string())),
    };
    if let Some(book_id) = request.book_id {
        let book_id = match bind_id(book_id, "Book id") {
            Ok(id) => id,
            Err(error) => return error_response(error),
        };
        let exists = match db
            .prepare("SELECT id FROM books WHERE id = ?")
            .bind_refs(&book_id)
        {
            Ok(statement) => match statement.first::<i32>(None).await {
                Ok(value) => value.is_some(),
                Err(error) => return error_response(AppError::Database(error.to_string())),
            },
            Err(error) => return error_response(AppError::Database(error.to_string())),
        };
        if !exists {
            return error_response(AppError::NotFound);
        }
    }

    let object_key_value = D1Type::Text(&key);
    let book_id_value = request.book_id.map(|id| bind_id(id, "Book id")).transpose();
    let book_id_value = match book_id_value {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let book_id_value = book_id_value.unwrap_or(D1Type::Null);
    let content_type_value = D1Type::Text(&content_type);
    let extension_value = D1Type::Text(&extension);
    let size_value = match i32::try_from(request.size) {
        Ok(size) => D1Type::Integer(size),
        Err(_) => {
            return error_response(AppError::Validation(
                "Cover size is out of range".to_string(),
            ));
        }
    };
    let created_at = Date::now().as_millis().to_string();
    let created_at_value = D1Type::Text(&created_at);
    let statement = db.prepare(
        "INSERT INTO cover_objects
         (object_key, book_id, content_type, extension, expected_size, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    );
    let statement = match statement.bind_refs([
        &object_key_value,
        &book_id_value,
        &content_type_value,
        &extension_value,
        &size_value,
        &created_at_value,
    ]) {
        Ok(statement) => statement,
        Err(error) => return error_response(AppError::Database(error.to_string())),
    };
    if let Err(error) = statement.run().await {
        return error_response(AppError::Database(error.to_string()));
    }

    Response::from_json(&CoverInitResponse {
        object_key: key,
        upload_url,
        expires_in: crate::wasabi::UPLOAD_URL_TTL_SECONDS,
        content_type,
    })
}

pub async fn complete(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CoverCompleteRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    if !is_safe_object_key(&request.object_key) {
        return Ok(bad_request("invalid object_key"));
    }
    let db = ctx.d1("DB")?;
    let key = D1Type::Text(&request.object_key);
    let record = match db
        .prepare(
            "SELECT object_key, book_id, content_type, extension, expected_size, status
             FROM cover_objects WHERE object_key = ?",
        )
        .bind_refs(&key)
    {
        Ok(statement) => match statement.first::<CoverObjectRecord>(None).await {
            Ok(Some(record)) => record,
            Ok(None) => return error_response(AppError::NotFound),
            Err(error) => return error_response(AppError::Database(error.to_string())),
        },
        Err(error) => return error_response(AppError::Database(error.to_string())),
    };
    let config = match WasabiConfig::from_env(&ctx.env) {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(storage_message(error))),
    };
    let storage = WasabiStorage::new(config);
    let metadata = match storage.head_object(&record.object_key).await {
        Ok(metadata) => metadata,
        Err(error) => return error_response(error),
    };
    if metadata.content_length != Some(record.expected_size as u64) {
        return error_response(AppError::Conflict(
            "Wasabi object size does not match upload metadata".to_string(),
        ));
    }
    if metadata
        .content_type
        .as_deref()
        .is_some_and(|value| !value.eq_ignore_ascii_case(&record.content_type))
    {
        return error_response(AppError::Conflict(
            "Wasabi object content type does not match upload metadata".to_string(),
        ));
    }

    let status = D1Type::Text("complete");
    db.prepare("UPDATE cover_objects SET status = ? WHERE object_key = ?")
        .bind_refs([&status, &key])
        .map_err(|error| worker::Error::RustError(error.to_string()))?
        .run()
        .await
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    let download_url = match storage.temporary_get_url(&record.object_key).await {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    let _ = (&record.extension, &record.status);
    Response::from_json(&CoverCompleteResponse {
        object_key: record.object_key,
        book_id: record.book_id,
        download_url,
    })
}

fn is_safe_object_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 512
        && key.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.=".contains(&byte))
        })
}

fn validate_cover_metadata(request: &CoverInitRequest) -> Result<(String, String), AppError> {
    if request.size <= 0 || request.size > MAX_COVER_BYTES {
        return Err(AppError::Validation(
            "Cover size must be between 1 byte and 10 MiB".to_string(),
        ));
    }
    let content_type = request.content_type.trim().to_ascii_lowercase();
    let extension = request
        .extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    let valid = matches!(
        (content_type.as_str(), extension.as_str()),
        ("image/jpeg", "jpg" | "jpeg") | ("image/png", "png") | ("image/webp", "webp")
    );
    if !valid {
        return Err(AppError::Validation(
            "Unsupported cover content type or extension".to_string(),
        ));
    }
    Ok((content_type, extension))
}

fn bind_id(id: i64, label: &str) -> Result<D1Type<'static>, AppError> {
    let id =
        i32::try_from(id).map_err(|_| AppError::Validation(format!("{label} is out of range")))?;
    if id <= 0 {
        return Err(AppError::Validation(format!("{label} must be positive")));
    }
    Ok(D1Type::Integer(id))
}

fn storage_message(error: worker::Error) -> String {
    format!("Wasabi configuration error: {error}")
}
