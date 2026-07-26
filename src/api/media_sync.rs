use crate::AppState;
use crate::media_sync::{self, MediaSyncConfig};
use axum::{Json, extract::State, http::StatusCode};

type ApiResult = Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)>;

pub async fn run_media_sync(State(state): State<AppState>) -> ApiResult {
    let config = MediaSyncConfig::load(
        &state.db,
        (*state.images_dir).clone(),
        (*state.audio_dir).clone(),
        (*state.epubs_dir).clone(),
    );

    if let Err(e) = config.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "ok": false,
                "error": e,
            })),
        ));
    }

    match media_sync::perform_media_sync(&config).await {
        Ok(summary) => Ok(Json(serde_json::to_value(summary).unwrap_or_else(
            |_| serde_json::json!({"ok": false, "error": "Failed to serialize summary"}),
        ))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "ok": false,
                "error": e,
            })),
        )),
    }
}
