use axum::{
    Json, extract::{Multipart, Path, State},
};
use tokio::fs;

pub async fn list_tracks(
    State(state): State<crate::AppState>,
    Path(book_id): Path<i64>,
) -> Result<Json<Vec<crate::db_models::Track>>, axum::http::StatusCode> {
    let db = state.db.clone();
    let tracks = tokio::task::spawn_blocking(move || db.list_tracks(book_id))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(tracks))
}

pub async fn update_track(
    State(state): State<crate::AppState>,
    Path((_book_id, track_id)): Path<(i64, i64)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<String>, axum::http::StatusCode> {
    let title = body["title"].as_str().unwrap_or("").to_string();
    let duration = body["duration"].as_str().map(|s| s.to_string());
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        db.update_track(track_id, &title, duration.as_deref())
    })
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json("ok".into()))
}

pub async fn upload_track_audio(
    State(state): State<crate::AppState>,
    Path((_book_id, track_id)): Path<(i64, i64)>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let audio_dir = state.audio_dir.as_str();

    let mut file_hash: Option<String> = None;
    let mut file_name: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?
    {
        let name = field.file_name().unwrap_or("unknown").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

        let (saved_name, _ext) =
            crate::external::save_uploaded_audio(&data, &name, &audio_dir)
                .map_err(|e| {
                    tracing::warn!(track_id, "Audio save failed: {}", e);
                    axum::http::StatusCode::BAD_REQUEST
                })?;

        file_hash = Some(saved_name);
        file_name = Some(name);
    }

    let (hash, fname) = match (file_hash, file_name) {
        (Some(h), Some(n)) => (h, n),
        _ => return Err(axum::http::StatusCode::BAD_REQUEST),
    };

    let db = state.db.clone();
    let t_hash = hash.clone();
    let t_name = fname.clone();
    tokio::task::spawn_blocking(move || {
        db.update_track_audio(track_id, Some(&t_hash), Some(&t_name))
    })
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "file_hash": hash,
        "file_name": fname,
    })))
}

pub async fn delete_track_audio(
    State(state): State<crate::AppState>,
    Path((_book_id, track_id)): Path<(i64, i64)>,
) -> Result<Json<String>, axum::http::StatusCode> {
    let db = state.db.clone();
    let audio_dir = state.audio_dir.as_str();

    let tracks = tokio::task::spawn_blocking(move || db.list_tracks(track_id))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(track) = tracks.first() {
        if let Some(ref hash) = track.file_hash {
            let path = format!("{}/{}", audio_dir, hash);
            let _ = fs::remove_file(&path).await;
        }
    }

    let db = state.db.clone();
    tokio::task::spawn_blocking(move || db.update_track_audio(track_id, None, None))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json("ok".into()))
}
