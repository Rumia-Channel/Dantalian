use crate::AppState;
use crate::db::{CdWithTracks, NewCd};
use crate::external;
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use base64::Engine;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};
use std::sync::Arc;
use tokio::fs;

use super::books::ApiError;

#[derive(Deserialize)]
pub struct CdRegisterRequest {
    pub jan: String,
    pub parent_book_id: Option<i64>,
    pub media_type: Option<String>,
    pub series_id: Option<i64>,
}

pub async fn cd_register(
    State(state): State<AppState>,
    Json(req): Json<CdRegisterRequest>,
) -> Result<(StatusCode, Json<CdWithTracks>), ApiError> {
    let jan = req.jan.replace('-', "").replace(' ', "");
    if jan.len() < 8 || jan.len() > 14 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid JAN length"})),
        ));
    }

    if let Ok(Some(existing)) = state.db.find_by_cd_jan(&jan) {
        let tracks = state.db.list_tracks_for_cd(existing.id).unwrap_or_default();
        return Ok((
            StatusCode::OK,
            Json(CdWithTracks {
                cd: existing,
                tracks,
            }),
        ));
    }

    let cd_info = match external::lookup_cd(&state.client, &jan, &state.images_dir).await {
        ok @ Ok(_) => ok,
        Err(e) => {
            tracing::warn!("CD lookup failed: {}. Retrying...", e);
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            external::lookup_cd(&state.client_ipv4, &jan, &state.images_dir).await
        }
    }
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": e})),
            )
        })?;

    let cd_info = cd_info.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "CD not found for this JAN"})),
        )
    })?;

    let new_cd = NewCd {
        jan: Some(jan),
        title: cd_info.title,
        artist: cd_info.artist,
        publisher: cd_info.publisher,
        label: cd_info.label,
        catalog_number: cd_info.catalog_number,
        publish_date: cd_info.publish_date,
        cover_url: cd_info.cover_url,
        description: None,
        disc_count: cd_info.disc_count,
        tracks: Some(cd_info.tracks),
        parent_book_id: req.parent_book_id,
        media_type: req.media_type.clone(),
        series_id: req.series_id,
    };

    let cd = state.db.insert_cd(&new_cd).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    if let Some(ref tracks) = new_cd.tracks {
        let _ = state.db.insert_tracks_batch_for_cd(cd.id, tracks);
    }

    let tracks = state.db.list_tracks_for_cd(cd.id).unwrap_or_default();
    Ok((
        StatusCode::CREATED,
        Json(CdWithTracks { cd, tracks }),
    ))
}

pub async fn list_cds(
    State(state): State<AppState>,
) -> Result<Json<Vec<CdWithTracks>>, StatusCode> {
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let cds = db.list_cds()?;
        let mut result = Vec::new();
        for cd in cds {
            let tracks = db.list_tracks_for_cd(cd.id).unwrap_or_default();
            result.push(CdWithTracks { cd, tracks });
        }
        Ok::<_, rusqlite::Error>(result)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(result))
}

pub async fn delete_cd(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    if state
        .db
        .delete_cd(id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "CD not found"})),
        ))
    }
}

pub async fn update_cd(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, ApiError> {
    let jan = body["jan"].as_str().map(|s| s.to_string());
    let title = body["title"].as_str().unwrap_or("").to_string();
    if title.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Title is required"})),
        ));
    }
    state
        .db
        .update_cd(
            id,
            jan.as_deref(),
            title.trim(),
            body["artist"].as_str(),
            body["publisher"].as_str(),
            body["label"].as_str(),
            body["catalog_number"].as_str(),
            body["publish_date"].as_str(),
            body["cover_url"].as_str(),
            body["description"].as_str(),
            body["disc_count"].as_i64(),
            body["parent_book_id"].as_i64(),
            body["media_type"].as_str(),
            body["series_id"].as_i64(),
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_cd_tracks(
    State(state): State<AppState>,
    Path(cd_id): Path<i64>,
) -> Result<Json<Vec<crate::db_models::Track>>, StatusCode> {
    let db = state.db.clone();
    let tracks = tokio::task::spawn_blocking(move || db.list_tracks_for_cd(cd_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(tracks))
}

pub async fn update_cd_track(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<String>, StatusCode> {
    let title = body["title"].as_str().unwrap_or("").to_string();
    let duration = body["duration"].as_str().map(|s| s.to_string());
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        db.update_track(track_id, &title, duration.as_deref())
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json("ok".into()))
}

pub async fn upload_cd_track_audio(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let audio_dir = Arc::clone(&state.images_dir);
    let audio_dir = audio_dir.replace("/images", "/audio");
    fs::create_dir_all(&audio_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut file_hash: Option<String> = None;
    let mut file_name: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("unknown").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let mut hasher = Sha3_256::new();
        hasher.update(&data);
        let hash = hasher.finalize();
        let hash_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            hash.as_slice(),
        );
        let ext = name.rsplit('.').next().unwrap_or("mp3");
        let save_name = format!("{}.{}", hash_b64, ext);
        let save_path = format!("{}/{}", audio_dir, save_name);

        fs::write(&save_path, &data)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        file_hash = Some(save_name.clone());
        file_name = Some(name);
    }

    let (hash, fname) = match (file_hash, file_name) {
        (Some(h), Some(n)) => (h, n),
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let db = state.db.clone();
    let t_hash = hash.clone();
    let t_name = fname.clone();
    tokio::task::spawn_blocking(move || {
        db.update_track_audio(track_id, Some(&t_hash), Some(&t_name))
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "file_hash": hash,
        "file_name": fname,
    })))
}

pub async fn delete_cd_track_audio(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
) -> Result<Json<String>, StatusCode> {
    let db = state.db.clone();
    let audio_dir = Arc::clone(&state.images_dir);
    let audio_dir = audio_dir.replace("/images", "/audio");

    let tracks = tokio::task::spawn_blocking(move || db.list_tracks(track_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(track) = tracks.first() {
        if let Some(ref hash) = track.file_hash {
            let path = format!("{}/{}", audio_dir, hash);
            let _ = fs::remove_file(&path).await;
        }
    }

    let db = state.db.clone();
    tokio::task::spawn_blocking(move || db.update_track_audio(track_id, None, None))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json("ok".into()))
}

pub async fn upload_cd_cover(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let cd = state
        .db
        .find_cd_by_id(id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "CD not found"})),
            )
        })?;

    let mut data: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "cover" {
            let ct = field.content_type().unwrap_or("image/jpeg").to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                })?
                .to_vec();
            if bytes.len() > 10 * 1024 * 1024 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "File too large (max 10MB)"})),
                ));
            }
            data = Some(bytes);
            content_type = Some(ct);
        }
    }

    let bytes = data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No file uploaded"})),
        )
    })?;

    let ct = content_type.unwrap_or("image/jpeg".to_string());
    let ext = match ct.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "jpg",
    };

    let hash = Sha3_256::digest(&bytes);
    let filename = format!(
        "{}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash),
        ext
    );
    let filepath = std::path::Path::new(state.images_dir.as_str()).join(&filename);

    std::fs::write(&filepath, &bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    state
        .db
        .update_cd_cover_url(id, Some(&filename))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    if let Some(old) = &cd.cover_url {
        if old != &filename {
            let old_path = std::path::Path::new(state.images_dir.as_str()).join(old);
            let _ = std::fs::remove_file(old_path);
        }
    }

    Ok(Json(serde_json::json!({"cover_url": filename})))
}

pub async fn delete_cd_cover(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let cd = state
        .db
        .find_cd_by_id(id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "CD not found"})),
            )
        })?;

    if let Some(old) = &cd.cover_url {
        let old_path = std::path::Path::new(state.images_dir.as_str()).join(old);
        let _ = std::fs::remove_file(old_path);
    }

    state.db.update_cd_cover_url(id, None).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    Ok(StatusCode::NO_CONTENT)
}
