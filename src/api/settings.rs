use axum::{Json, extract::State, http::StatusCode};
use std::collections::{HashMap, HashSet};

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

fn is_allowed_key(key: &str) -> bool {
    ALLOWED_SETTING_KEYS.contains(&key)
}

fn is_secret_key(key: &str) -> bool {
    SECRET_SETTING_KEYS.contains(&key)
}

fn public_settings(db: &crate::db::Db) -> HashMap<String, String> {
    let mut public = HashMap::new();
    for (key, value) in db.get_all_settings() {
        if !is_allowed_key(&key) {
            continue;
        }
        if is_secret_key(&key) {
            if !value.is_empty() {
                public.insert(format!("{key}.__configured"), "true".to_string());
            }
        } else {
            public.insert(key, value);
        }
    }
    public
}

fn validate_setting(key: &str, value: &str) -> Result<(), String> {
    if value.chars().any(char::is_control) {
        return Err(format!("{key} に制御文字は指定できません"));
    }
    if value.len() > 4096 {
        return Err(format!("{key} は4096文字以内で指定してください"));
    }

    match key {
        "upload.cover_max_mb" | "upload.audio_max_mb" | "upload.file_max_mb" => {
            let megabytes = value
                .trim()
                .parse::<u64>()
                .map_err(|_| format!("{key} は整数で指定してください"))?;
            if !(1..=4096).contains(&megabytes) {
                return Err(format!("{key} は1〜4096MBで指定してください"));
            }
        }
        "backup.retention" => {
            let retention = value
                .trim()
                .parse::<u64>()
                .map_err(|_| "backup.retention は整数で指定してください".to_string())?;
            if !(1..=365).contains(&retention) {
                return Err("backup.retention は1〜365で指定してください".to_string());
            }
        }
        "audio.data_saver.enabled" | "backup.enabled" | "media_sync.enabled" => {
            if !matches!(value.trim(), "true" | "false") {
                return Err(format!("{key} は true または false で指定してください"));
            }
        }
        "audio.data_saver.extensions" => {
            for extension in value.split(',').map(str::trim).filter(|v| !v.is_empty()) {
                if crate::audio_encoding::normalize_extension(extension).is_none() {
                    return Err(format!("変換対象の拡張子が不正です: {extension}"));
                }
            }
        }
        "backup.dest_type" => {
            if !matches!(value.trim(), "local" | "webdav" | "s3") {
                return Err("backup.dest_type は local/webdav/s3 のいずれかです".to_string());
            }
        }
        "media_sync.types" => {
            let allowed = ["images", "audio", "epubs"];
            let mut types = HashSet::new();
            for media_type in value.split(',').map(str::trim).filter(|v| !v.is_empty()) {
                if !allowed.contains(&media_type) || !types.insert(media_type) {
                    return Err(format!("同期対象のメディア種別が不正です: {media_type}"));
                }
            }
        }
        "backup.schedule_time" | "media_sync.schedule_time" => {
            if !value.trim().is_empty() && value.parse::<chrono::NaiveTime>().is_err() {
                return Err(format!("{key} の時刻形式が不正です"));
            }
        }
        "backup.schedule_tz" | "media_sync.schedule_tz" => {
            if !value.trim().is_empty() && value.parse::<chrono_tz::Tz>().is_err() {
                return Err(format!("{key} のタイムゾーンが不正です"));
            }
        }
        _ => {}
    }
    Ok(())
}

pub async fn get_settings(State(state): State<crate::AppState>) -> Json<HashMap<String, String>> {
    Json(public_settings(&state.db))
}

pub async fn update_settings(
    State(state): State<crate::AppState>,
    Json(settings): Json<HashMap<String, String>>,
) -> Result<Json<HashMap<String, String>>, (StatusCode, Json<serde_json::Value>)> {
    let audio_encoding_settings_changed = settings.keys().any(|key| {
        matches!(
            key.as_str(),
            crate::audio_encoding::KEY_ENABLED | crate::audio_encoding::KEY_EXTENSIONS
        )
    });
    for (key, value) in &settings {
        if !is_allowed_key(key) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("未知の設定キーです: {key}")})),
            ));
        }
        if let Err(error) = validate_setting(key, value) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": error})),
            ));
        }
    }
    state.db.set_settings(&settings).map_err(|error| {
        tracing::error!("Failed to save settings: {}", error);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "設定をデータベースへ保存できませんでした"})),
        )
    })?;
    if audio_encoding_settings_changed {
        state.audio_encoding_notify.notify_one();
    }
    Ok(Json(public_settings(&state.db)))
}

#[cfg(test)]
mod tests {
    use super::validate_setting;

    #[test]
    fn validates_upload_limits_and_media_types() {
        assert!(validate_setting("upload.audio_max_mb", "500").is_ok());
        assert!(validate_setting("upload.audio_max_mb", "0").is_err());
        assert!(validate_setting("upload.audio_max_mb", "5000").is_err());
        assert!(validate_setting("media_sync.types", "images,audio").is_ok());
        assert!(validate_setting("media_sync.types", "audio,audio").is_err());
        assert!(validate_setting("media_sync.types", "videos").is_err());
    }

    #[test]
    fn rejects_invalid_boolean_and_schedule_values() {
        assert!(validate_setting("backup.enabled", "true").is_ok());
        assert!(validate_setting("backup.enabled", "yes").is_err());
        assert!(validate_setting("backup.schedule_time", "03:30").is_ok());
        assert!(validate_setting("backup.schedule_time", "not-a-time").is_err());
        assert!(validate_setting("backup.schedule_tz", "Asia/Tokyo").is_ok());
        assert!(validate_setting("backup.schedule_tz", "Mars/Nope").is_err());
    }
}
