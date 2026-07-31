use crate::api::upload_chunks::{ChunkQuery, StoreResult};
use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
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
    // The track modal only sends the user-editable fields; preserve everything the
    // audio extraction produced (artist/album/cover/ReplayGain/file info/...).
    let db = state.db.clone();
    let existing = match tokio::task::spawn_blocking(move || db.get_track_metadata(track_id)).await
    {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => {
            tracing::error!(track_id, "book track_metadata read failed: {}", e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(e) => {
            tracing::error!(track_id, "book track_metadata task failed: {}", e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let mut merged = crate::external::audio_meta::TrackMetadata::from_json(&body);
    if let Some(ex) = existing {
        merged.artist = ex.artist;
        merged.album = ex.album;
        merged.album_artist = ex.album_artist;
        merged.year = ex.year;
        merged.genre = ex.genre;
        merged.composer = ex.composer;
        merged.publisher = ex.publisher;
        merged.label = ex.label;
        // 歌詞はモーダルで編集可能なので、body に含まれるときはそちらを優先する。
        if body.get("lyrics").is_none() {
            merged.lyrics = ex.lyrics;
        }
        merged.cover_mime = ex.cover_mime;
        merged.cover_data = ex.cover_data;
        merged.replay_gain_track_gain_db = ex.replay_gain_track_gain_db;
        merged.replay_gain_track_peak = ex.replay_gain_track_peak;
        merged.replay_gain_album_gain_db = ex.replay_gain_album_gain_db;
        merged.replay_gain_album_peak = ex.replay_gain_album_peak;
        merged.file_type = ex.file_type;
        merged.raw_size_bytes = ex.raw_size_bytes;
    }
    let db = state.db.clone();
    let join_result =
        tokio::task::spawn_blocking(move || db.upsert_track_metadata(track_id, &merged)).await;
    if let Err(e) = join_result.as_ref() {
        tracing::error!(track_id, "book track_metadata task failed: {}", e);
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }
    if let Err(e) = join_result.as_ref().unwrap().as_ref() {
        tracing::error!(track_id, "book track_metadata upsert failed: {}", e);
        return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }

    if let Some(artists_v) = body.get("artists") {
        // フロントは作者IDの配列を送る (例: [3, 7])
        let ids: Vec<i64> = if let Some(arr) = artists_v.as_array() {
            arr.iter().filter_map(|v| v.as_i64()).collect()
        } else {
            Vec::new()
        };
        let db = state.db.clone();
        let assign = tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
            db.replace_track_authors(track_id, &ids)?;
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
    Query(chunk): Query<ChunkQuery>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let audio_dir = state.audio_dir.as_str();
    let chunk_info = chunk
        .validate()
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

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
        let audio_max = crate::api::upload_limit_bytes(
            &state,
            crate::api::KEY_UPLOAD_AUDIO_MB,
            crate::api::AUDIO_MAX_BYTES,
        );
        let (saved_name, _ext) = match chunk_info.as_ref() {
            Some(info) => match crate::api::upload_chunks::store_chunk(
                state.uploads_dir.as_str(),
                "audio",
                info.clone(),
                &data,
            )
            .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?
            {
                StoreResult::Partial { part, total_parts } => {
                    return Ok(Json(serde_json::json!({
                        "chunked": true,
                        "complete": false,
                        "part": part,
                        "total_parts": total_parts,
                    })));
                }
                StoreResult::Complete { path, cleanup_dir } => {
                    let result = crate::external::save_uploaded_audio_path(
                        &path, &name, audio_dir, audio_max,
                    );
                    let _ = std::fs::remove_dir_all(cleanup_dir);
                    result
                }
            }
            .map_err(|e| {
                tracing::warn!(track_id, "Audio save failed: {}", e);
                axum::http::StatusCode::BAD_REQUEST
            })?,
            None => crate::external::save_uploaded_audio(&data, &name, audio_dir, audio_max)
                .map_err(|e| {
                    tracing::warn!(track_id, "Audio save failed: {}", e);
                    axum::http::StatusCode::BAD_REQUEST
                })?,
        };

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
    state.audio_encoding_notify.notify_one();

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
    Path((book_id, track_id)): Path<(i64, i64)>,
) -> Result<Json<String>, axum::http::StatusCode> {
    let db = state.db.clone();
    let audio_dir = state.audio_dir.as_str();

    let track = tokio::task::spawn_blocking(move || db.find_track_by_id(track_id))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if track.as_ref().map(|track| track.book_id) != Some(book_id) {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }
    if let Some(track) = track {
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
