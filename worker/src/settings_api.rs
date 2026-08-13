use std::collections::{HashMap, HashSet};

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

fn is_secret(key: &str) -> bool {
    SECRET_SETTING_KEYS.contains(&key)
}

fn validate(key: &str, value: &str) -> std::result::Result<(), AppError> {
    if !is_allowed(key) {
        return Err(AppError::Validation(format!("unknown setting key: {key}")));
    }
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(AppError::Validation(format!(
            "invalid value for setting: {key}"
        )));
    }
    match key {
        "upload.cover_max_mb" | "upload.audio_max_mb" | "upload.file_max_mb" => {
            let value = value
                .trim()
                .parse::<u64>()
                .map_err(|_| AppError::Validation(format!("{key} must be an integer")))?;
            if !(1..=4096).contains(&value) {
                return Err(AppError::Validation(format!(
                    "{key} must be between 1 and 4096"
                )));
            }
        }
        "backup.retention" => {
            let value = value
                .trim()
                .parse::<u64>()
                .map_err(|_| AppError::Validation("backup.retention must be an integer".into()))?;
            if !(1..=365).contains(&value) {
                return Err(AppError::Validation(
                    "backup.retention must be between 1 and 365".into(),
                ));
            }
        }
        "audio.data_saver.enabled" | "backup.enabled" | "media_sync.enabled" => {
            if !matches!(value.trim(), "true" | "false") {
                return Err(AppError::Validation(format!("{key} must be true or false")));
            }
        }
        "backup.dest_type" => {
            if !matches!(value.trim(), "local" | "webdav" | "s3") {
                return Err(AppError::Validation(
                    "backup.dest_type must be local, webdav, or s3".into(),
                ));
            }
        }
        "media_sync.types" => {
            let allowed = ["images", "audio", "epubs"];
            let mut seen = HashSet::new();
            for value in value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if !allowed.contains(&value) || !seen.insert(value) {
                    return Err(AppError::Validation(format!(
                        "invalid media sync type: {value}"
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
        if !is_allowed(&row.key) {
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
    Response::from_json(&settings)
}
