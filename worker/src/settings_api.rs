use std::collections::HashMap;

use dantalian::application::error::AppError;
use serde::Deserialize;
use worker::{D1Type, Request, Response, Result, RouteContext};

use crate::error::{error_response, parse_json};

const ALLOWED_SETTING_KEYS: &[&str] = &[
    "discogs_token",
    "musicbrainz_contact",
    "upload.cover_max_mb",
    "upload.audio_max_mb",
    "upload.file_max_mb",
    "audio.data_saver.enabled",
    "audio.data_saver.extensions",
    "backup.enabled",
    "backup.schedule_time",
    "backup.schedule_tz",
    "backup.retention",
    "backup.dest_type",
    "backup.local_path",
    "backup.webdav_url",
    "backup.webdav_user",
    "backup.webdav_pass",
    "backup.s3_endpoint",
    "backup.s3_region",
    "backup.s3_bucket",
    "backup.s3_access_key",
    "backup.s3_secret_key",
    "backup.s3_prefix",
    "media_sync.enabled",
    "media_sync.types",
    "media_sync.schedule_time",
    "media_sync.schedule_tz",
    "media_sync.s3_endpoint",
    "media_sync.s3_region",
    "media_sync.s3_bucket",
    "media_sync.s3_access_key",
    "media_sync.s3_secret_key",
    "media_sync.s3_prefix",
];

const SECRET_SETTING_KEYS: &[&str] = &[
    "discogs_token",
    "backup.webdav_pass",
    "backup.s3_access_key",
    "backup.s3_secret_key",
    "media_sync.s3_access_key",
    "media_sync.s3_secret_key",
];

#[derive(Debug, Deserialize)]
struct SettingRow {
    key: String,
    value: String,
}

fn db_error(error: worker::Error) -> worker::Error {
    error
}

fn is_allowed(key: &str) -> bool {
    ALLOWED_SETTING_KEYS.contains(&key)
}

fn is_worker_unsupported(key: &str) -> bool {
    matches!(
        key,
        "upload.cover_max_mb" | "upload.audio_max_mb" | "upload.file_max_mb"
    ) || key.starts_with("backup.")
        || key.starts_with("media_sync.")
}

fn is_secret(key: &str) -> bool {
    SECRET_SETTING_KEYS.contains(&key)
}

fn validate(key: &str, value: &str) -> std::result::Result<(), AppError> {
    if !is_allowed(key) {
        return Err(AppError::Validation(format!("unknown setting key: {key}")));
    }
    if is_worker_unsupported(key) {
        return Err(AppError::Validation(format!(
            "{key} is not configurable in Worker runtime"
        )));
    }
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(AppError::Validation(format!(
            "invalid value for setting: {key}"
        )));
    }
    match key {
        "audio.data_saver.enabled" => {
            if !matches!(value.trim(), "true" | "false") {
                return Err(AppError::Validation(format!("{key} must be true or false")));
            }
        }
        "audio.data_saver.extensions" => {
            for extension in value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let extension = extension.trim_start_matches('.');
                if extension.is_empty()
                    || extension.len() > 10
                    || !extension
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
                {
                    return Err(AppError::Validation(format!(
                        "invalid audio data saver extension: {extension}"
                    )));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

async fn public_settings(db: &worker::D1Database) -> Result<HashMap<String, String>> {
    let rows = db
        .prepare("SELECT key, value FROM settings")
        .all()
        .await
        .map_err(db_error)?
        .results::<SettingRow>()
        .map_err(db_error)?;
    let mut settings = HashMap::new();
    for row in rows {
        if !is_allowed(&row.key) || is_worker_unsupported(&row.key) {
            continue;
        }
        if is_secret(&row.key) {
            if !row.value.is_empty() {
                settings.insert(format!("{}.__configured", row.key), "true".to_string());
            }
        } else {
            settings.insert(row.key, row.value);
        }
    }
    Ok(settings)
}

pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let settings = match public_settings(&ctx.d1("DB")?).await {
        Ok(settings) => settings,
        Err(error) => return Err(error),
    };
    Response::from_json(&settings)
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let settings = match parse_json::<HashMap<String, String>>(&mut req).await {
        Ok(settings) => settings,
        Err(response) => return Ok(response),
    };
    let data_saver_changed = settings.keys().any(|key| {
        matches!(
            key.as_str(),
            "audio.data_saver.enabled" | "audio.data_saver.extensions"
        )
    });
    for (key, value) in &settings {
        if let Err(error) = validate(key, value) {
            return error_response(error);
        }
    }
    let db = ctx.d1("DB")?;
    for (key, value) in settings {
        let key_value = D1Type::Text(&key);
        let value_value = D1Type::Text(&value);
        db.prepare(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind_refs([&key_value, &value_value])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    }
    let settings = public_settings(&db).await?;
    if data_saver_changed {
        if let Err(error) = crate::audio_job_api::enqueue_data_saver_jobs(&ctx.env).await {
            worker::console_error!("data saver job scheduling deferred: {error}");
        }
    }
    Response::from_json(&settings)
}
