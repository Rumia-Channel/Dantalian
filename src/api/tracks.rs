use axum::{
    Json,
    extract::{Multipart, Path, State},
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

pub async fn get_book_track_metadata(
    State(state): State<crate::AppState>,
    Path((_book_id, track_id)): Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let db = state.db.clone();
    let meta =
        tokio::task::spawn_blocking(move || db.get_track_metadata_with_cd_inheritance(track_id))
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let json = serde_json::to_value(&meta).map_err(|e| {
        tracing::error!(track_id, "book track_metadata serialize failed: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json))
}

pub async fn put_book_track_metadata(
    State(state): State<crate::AppState>,
    Path((_book_id, track_id)): Path<(i64, i64)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<String>, axum::http::StatusCode> {
    let meta = crate::external::audio_meta::TrackMetadata::from_json(&body);
    let db = state.db.clone();
    let join_result =
        tokio::task::spawn_blocking(move || db.upsert_track_metadata(track_id, &meta)).await;
    if let Err(e) = join_result.as_ref() {
        tracing::error!(track_id, "book track_metadata task failed: {}", e);
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }
    if let Err(e) = join_result.as_ref().unwrap().as_ref() {
        tracing::error!(track_id, "book track_metadata upsert failed: {}", e);
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }

    if let Some(artists_v) = body.get("artists") {
        let names: Vec<String> = if let Some(arr) = artists_v.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            Vec::new()
        };
        let db = state.db.clone();
        let assign = tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            if names.is_empty() {
                db.replace_track_authors(track_id, &[])?;
            } else {
                let ids = db.ensure_authors_for_names(&names)?;
                db.replace_track_authors(track_id, &ids)?;
            }
            Ok(())
        })
        .await;
        if let Err(e) = assign.as_ref() {
            tracing::error!(track_id, "book track authors assign task failed: {}", e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
        if let Err(e) = assign.as_ref().unwrap().as_ref() {
            tracing::error!(track_id, "book track authors assign failed: {}", e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json("ok".into()))
}

pub async fn update_track(
    State(state): State<crate::AppState>,
    Path((_book_id, track_id)): Path<(i64, i64)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<String>, axum::http::StatusCode> {
    let title = body
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let duration = body
        .get("duration")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    if title.is_none() && duration.is_none() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        db.update_track(track_id, title.as_deref(), duration.as_deref())
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

        let (saved_name, _ext) = crate::external::save_uploaded_audio(&data, &name, &audio_dir)
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

    let metadata = extract_and_save_metadata(&state, track_id, &hash).await;

    Ok(Json(serde_json::json!({
        "file_hash": hash,
        "file_name": fname,
        "metadata": metadata,
    })))
}

async fn extract_and_save_metadata(
    state: &crate::AppState,
    track_id: i64,
    file_hash: &str,
) -> Option<serde_json::Value> {
    let path = std::path::PathBuf::from(state.audio_dir.as_str()).join(file_hash);
    let path_for_task = path.clone();

    let extracted = match tokio::task::spawn_blocking(move || {
        crate::external::audio_meta::extract(&path_for_task)
    })
    .await
    {
        Ok(Ok(meta)) => meta,
        Ok(Err(e)) => {
            tracing::debug!(
                track_id,
                file_hash,
                "Audio metadata extraction failed: {}",
                e
            );
            return None;
        }
        Err(e) => {
            tracing::warn!(track_id, "Metadata extraction task failed: {}", e);
            return None;
        }
    };

    let db = state.db.clone();
    let meta_clone = extracted.clone();
    if tokio::task::spawn_blocking(move || db.upsert_track_metadata(track_id, &meta_clone))
        .await
        .map_err(|e| {
            tracing::warn!(track_id, "Metadata save task failed: {}", e);
        })
        .and_then(|r| {
            r.map_err(|e| {
                tracing::warn!(track_id, "Metadata upsert failed: {}", e);
            })
        })
        .is_err()
    {
        return None;
    }

    serde_json::to_value(&extracted).ok()
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
