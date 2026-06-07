use crate::AppState;
use crate::db::{CdWithTracks, NewCd};
use crate::db_models::CdMetadata;
use crate::external;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use axum_extra::extract::Multipart;
use base64::Engine;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};
use tokio::fs;

use super::books::ApiError;

#[derive(Deserialize)]
pub struct CdRegisterRequest {
    pub jan: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub publisher: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub publish_date: Option<String>,
    pub description: Option<String>,
    pub disc_count: Option<i64>,
    pub volume: Option<String>,
    pub parent_book_id: Option<i64>,
    pub media_type: Option<String>,
    pub series_id: Option<i64>,
    pub manual: Option<bool>,
}

pub async fn cd_register(
    State(state): State<AppState>,
    Json(req): Json<CdRegisterRequest>,
) -> Result<(StatusCode, Json<CdWithTracks>), ApiError> {
    let is_manual = req.manual.unwrap_or(false) || req.title.as_ref().map(|t| !t.trim().is_empty()).unwrap_or(false);

    if is_manual {
        let title = req.title.ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Title is required for manual registration"})))
        })?;
        let jan = req.jan.as_ref().map(|j| j.replace('-', "").replace(' ', "")).filter(|j| !j.is_empty());

        if let Some(ref j) = jan {
            if j.len() >= 8 {
                if let Ok(Some(existing)) = state.db.find_by_cd_jan(j) {
                    let tracks = state.db.list_tracks_for_cd(existing.id).unwrap_or_default();
                    let authors = state.db.get_cd_authors(existing.id).unwrap_or_default();
                    return Ok((StatusCode::OK, Json(CdWithTracks { cd: existing, tracks, authors })));
                }
            }
        }

        let new_cd = NewCd {
            jan,
            title,
            artist: req.artist,
            publisher: req.publisher,
            label: req.label,
            catalog_number: req.catalog_number,
            publish_date: req.publish_date,
            cover_url: None,
            description: req.description,
            disc_count: req.disc_count,
            volume: req.volume,
            tracks: None,
            parent_book_id: req.parent_book_id,
            media_type: req.media_type.clone(),
            series_id: req.series_id,
        };

        let cd = state.db.insert_cd(&new_cd).map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

        let tracks = state.db.list_tracks_for_cd(cd.id).unwrap_or_default();
        let authors = state.db.get_cd_authors(cd.id).unwrap_or_default();
        return Ok((StatusCode::CREATED, Json(CdWithTracks { cd, tracks, authors })));
    }

    let jan = req.jan.unwrap_or_default().replace('-', "").replace(' ', "");
    if jan.len() < 8 || jan.len() > 14 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid JAN length"})),
        ));
    }

    if let Ok(Some(existing)) = state.db.find_by_cd_jan(&jan) {
        let tracks = state.db.list_tracks_for_cd(existing.id).unwrap_or_default();
        let authors = state.db.get_cd_authors(existing.id).unwrap_or_default();
        return Ok((
            StatusCode::OK,
            Json(CdWithTracks {
                cd: existing,
                tracks,
                authors,
            }),
        ));
    }

    let cd_info = match external::lookup_cd(&state.client, &jan, &state.images_dir, &state.musicbrainz_contact).await {
        ok @ Ok(_) => ok,
        Err(e) => {
            tracing::warn!("MusicBrainz lookup failed: {}. Retrying...", e);
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            external::lookup_cd(&state.client_ipv4, &jan, &state.images_dir, &state.musicbrainz_contact).await
        }
    };

    let cd_info = match cd_info {
        Ok(Some(info)) => Some(info),
        Ok(None) => {
            tracing::info!("MusicBrainz returned no results for JAN={}", jan);
            None
        }
        Err(e) => {
            tracing::warn!("MusicBrainz lookup error: {}", e);
            None
        }
    };

    let cd_info = match cd_info {
        Some(info) => info,
        None => {
            if state.discogs_token.is_empty() {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "CD not found for this JAN"})),
                ));
            }
            tracing::info!("Falling back to Discogs for JAN={}", jan);
            match external::lookup_cd_discogs(&state.client, &jan, &state.discogs_token).await {
                Ok(Some(info)) => info,
                Ok(None) => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({"error": "CD not found for this JAN"})),
                    ));
                }
                Err(e) => {
                    return Err((
                        StatusCode::BAD_GATEWAY,
                        Json(serde_json::json!({"error": format!("Discogs error: {}", e)})),
                    ));
                }
            }
        }
    };

    let amazon_cover = match external::lookup_amazon_cover_for_jan(
        &state.client,
        &jan,
        &state.images_dir,
    )
    .await
    {
        Some(c) => Some(c),
        None => {
            tracing::warn!(jan = %jan, "Amazon cover not found, retrying with IPv4");
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            external::lookup_amazon_cover_for_jan(&state.client_ipv4, &jan, &state.images_dir)
                .await
        }
    };

    let fallback_cover = match cd_info.cover_url.as_deref() {
        Some(url) if url.starts_with("http://") || url.starts_with("https://") => {
            match external::download_image(&state.client, url, &state.images_dir, &[]).await {
                Ok(f) => Some(f),
                Err(e) => {
                    tracing::warn!(jan = %jan, url = %url, "Failed to download fallback cover: {}", e);
                    None
                }
            }
        }
        Some(url) => Some(url.to_string()),
        None => None,
    };

    let cover_url = amazon_cover.or(fallback_cover);

    let new_cd = NewCd {
        jan: Some(jan),
        title: cd_info.title,
        artist: cd_info.artist,
        publisher: cd_info.publisher,
        label: cd_info.label,
        catalog_number: cd_info.catalog_number,
        publish_date: cd_info.publish_date,
        cover_url,
        description: None,
        disc_count: cd_info.disc_count,
        volume: req.volume,
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
        Json(CdWithTracks { cd, tracks, authors: vec![] }),
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
            let authors = db.get_cd_authors(cd.id).unwrap_or_default();
            result.push(CdWithTracks { cd, tracks, authors });
        }
        Ok::<_, rusqlite::Error>(result)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(result))
}

#[derive(serde::Deserialize)]
pub struct MetadataSearchQuery {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn search_track_metadata(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<MetadataSearchQuery>,
) -> Result<Json<Vec<crate::db::track_metadata_search::MetadataSearchResult>>, StatusCode> {
    let db = state.db.clone();
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    let result = tokio::task::spawn_blocking(move || {
        db.search_tracks_by_metadata(
            q.artist.as_deref(),
            q.album.as_deref(),
            q.year,
            limit,
        )
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
    let title = body["title"].as_str().unwrap_or("").to_string();
    if title.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Title is required"})),
        ));
    }

    let existing = state
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

    let jan = body["jan"].as_str().or(existing.jan.as_deref()).map(|s| s.to_string());
    let artist = body["artist"].as_str().or(existing.artist.as_deref());
    let publisher = body["publisher"].as_str().or(existing.publisher.as_deref());
    let label = body["label"].as_str().or(existing.label.as_deref());
    let catalog_number = body["catalog_number"].as_str().or(existing.catalog_number.as_deref());
    let publish_date = body["publish_date"].as_str().or(existing.publish_date.as_deref());
    let description = body["description"].as_str().or(existing.description.as_deref());
    let disc_count = body["disc_count"].as_i64().or(existing.disc_count);
    let volume = body["volume"].as_str().or(existing.volume.as_deref()).map(|s| s.to_string());
    let parent_book_id = body["parent_book_id"].as_i64().or(existing.parent_book_id);
    let media_type = body["media_type"].as_str().or(existing.media_type.as_deref());
    let series_id = body["series_id"].as_i64().or(existing.series_id);

    state
        .db
        .update_cd(
            id,
            jan.as_deref(),
            title.trim(),
            artist,
            publisher,
            label,
            catalog_number,
            publish_date,
            description,
            disc_count,
            volume.as_deref(),
            parent_book_id,
            media_type,
            series_id,
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

pub async fn get_cd_track_metadata(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = state.db.clone();
    let join_result =
        tokio::task::spawn_blocking(move || db.get_track_metadata_with_cd_inheritance(track_id))
            .await;
    let meta = match join_result {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => {
            tracing::error!(track_id, "track_metadata read failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(e) => {
            tracing::error!(track_id, "track_metadata task failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let json = match serde_json::to_value(&meta) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(track_id, "track_metadata serialize failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    Ok(Json(json))
}

pub async fn put_cd_track_metadata(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<String>, StatusCode> {
    let meta = crate::external::audio_meta::TrackMetadata::from_json(&body);
    let db = state.db.clone();
    let join_result =
        tokio::task::spawn_blocking(move || db.upsert_track_metadata(track_id, &meta)).await;
    if let Err(e) = join_result.as_ref() {
        tracing::error!(track_id, "track_metadata task failed: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    if let Err(e) = join_result.as_ref().unwrap().as_ref() {
        tracing::error!(track_id, "track_metadata upsert failed: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
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
            tracing::error!(track_id, "track authors assign task failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        if let Err(e) = assign.as_ref().unwrap().as_ref() {
            tracing::error!(track_id, "track authors assign failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json("ok".into()))
}

pub async fn get_cd_metadata(
    State(state): State<AppState>,
    Path(cd_id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = state.db.clone();
    let join_result = tokio::task::spawn_blocking(move || db.get_cd_metadata(cd_id)).await;
    let meta = match join_result {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => {
            tracing::error!(cd_id, "cd_metadata read failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(e) => {
            tracing::error!(cd_id, "cd_metadata task failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let json = match serde_json::to_value(&meta) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(cd_id, "cd_metadata serialize failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    Ok(Json(json))
}

pub async fn put_cd_metadata(
    State(state): State<AppState>,
    Path(cd_id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<String>, StatusCode> {
    let meta = CdMetadata::from_json(cd_id, &body);
    let db = state.db.clone();
    let join_result =
        tokio::task::spawn_blocking(move || db.upsert_cd_metadata(cd_id, &meta)).await;
    match join_result {
        Ok(Ok(())) => Ok(Json("ok".into())),
        Ok(Err(e)) => {
            tracing::error!(cd_id, "cd_metadata upsert failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
        Err(e) => {
            tracing::error!(cd_id, "cd_metadata task failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn update_cd_track(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<String>, StatusCode> {
    let title = body.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
    let duration = body.get("duration").and_then(|v| v.as_str()).map(|s| s.to_string());
    if title.is_none() && duration.is_none() && body.get("disc_number").is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        db.update_track(track_id, title.as_deref(), duration.as_deref())
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(disc) = body.get("disc_number").and_then(|v| v.as_i64()) {
        let tn = body.get("track_number").and_then(|v| v.as_i64()).unwrap_or(1);
        let db2 = state.db.clone();
        tokio::task::spawn_blocking(move || {
            db2.update_track_position(track_id, disc, tn)
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json("ok".into()))
}

pub async fn add_cd_track(
    State(state): State<AppState>,
    Path(cd_id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<crate::db_models::Track>, StatusCode> {
    let track = crate::db_models::NewTrack {
        disc_number: body["disc_number"].as_i64().or(Some(1)),
        track_number: body["track_number"].as_i64().unwrap_or(1),
        title: body["title"].as_str().unwrap_or("").to_string(),
        duration: body["duration"].as_str().map(|s| s.to_string()),
    };
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || db.insert_track_for_cd(cd_id, &track))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        .map(Json)
}

pub async fn delete_cd_track(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
) -> Result<StatusCode, StatusCode> {
    let db = state.db.clone();
    let deleted = tokio::task::spawn_blocking(move || db.delete_track(track_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn upload_cd_track_audio(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let audio_dir = state.audio_dir.as_str();

    let mut file_hash: Option<String> = None;
    let mut file_name: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let name = field.file_name().unwrap_or("unknown").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let (saved_name, _ext) = external::save_uploaded_audio(&data, &name, &audio_dir)
            .map_err(|e| {
                tracing::warn!(track_id, cd_id = _cd_id, "Audio save failed: {}", e);
                StatusCode::BAD_REQUEST
            })?;

        file_hash = Some(saved_name);
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

    let cd_id_for_meta = state
        .db
        .list_tracks(track_id)
        .ok()
        .and_then(|v| v.into_iter().next())
        .and_then(|t| t.cd_id);
    let metadata = if let Some(cd_id) = cd_id_for_meta {
        extract_and_save_metadata(&state, track_id, cd_id, &hash).await
    } else {
        extract_and_save_track_only(&state, track_id, &hash).await
    };

    Ok(Json(serde_json::json!({
        "file_hash": hash,
        "file_name": fname,
        "metadata": metadata,
    })))
}

async fn extract_and_save_metadata(
    state: &AppState,
    track_id: i64,
    cd_id: i64,
    file_hash: &str,
) -> Option<serde_json::Value> {
    let path = std::path::PathBuf::from(state.audio_dir.as_str()).join(file_hash);
    let path_for_task = path.clone();

    let extracted = match tokio::task::spawn_blocking(move || {
        external::audio_meta::extract(&path_for_task)
    })
    .await
    {
        Ok(Ok(meta)) => meta,
        Ok(Err(e)) => {
            tracing::debug!(track_id, file_hash, "Audio metadata extraction failed: {}", e);
            return None;
        }
        Err(e) => {
            tracing::warn!(track_id, "Metadata extraction task failed: {}", e);
            return None;
        }
    };

    let db = state.db.clone();
    let track_meta = extracted.clone();
    let track_save = tokio::task::spawn_blocking(move || {
        db.upsert_track_metadata(track_id, &track_meta)
    })
    .await;

    if let Err(e) = track_save.as_ref() {
        tracing::warn!(track_id, "Track metadata save task failed: {}", e);
    } else if let Err(e) = track_save.as_ref().unwrap().as_ref() {
        tracing::warn!(track_id, "Track metadata upsert failed: {}", e);
    }

    let db = state.db.clone();
    let cd_meta = extracted.clone().into_cd_metadata(cd_id);
    let cd_save = tokio::task::spawn_blocking(move || {
        db.upsert_cd_metadata(cd_id, &cd_meta)
    })
    .await;

    if let Err(e) = cd_save.as_ref() {
        tracing::warn!(cd_id, "CD metadata save task failed: {}", e);
    } else if let Err(e) = cd_save.as_ref().unwrap().as_ref() {
        tracing::warn!(cd_id, "CD metadata upsert failed: {}", e);
    }

    if let Some(artist_str) = extracted.artist.clone() {
        let names = crate::external::audio_meta::split_artist_names(&artist_str);
        if !names.is_empty() {
            let db = state.db.clone();
            let _ = tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
                if let Ok(existing) = db.list_track_authors(track_id) {
                    if existing.is_empty() {
                        let ids = db.ensure_authors_for_names(&names)?;
                        db.replace_track_authors(track_id, &ids)?;
                    }
                }
                Ok(())
            })
            .await;
        }
    }

    serde_json::to_value(&extracted).ok()
}

async fn extract_and_save_track_only(
    state: &AppState,
    track_id: i64,
    file_hash: &str,
) -> Option<serde_json::Value> {
    let path = std::path::PathBuf::from(state.audio_dir.as_str()).join(file_hash);
    let path_for_task = path.clone();

    let extracted = match tokio::task::spawn_blocking(move || {
        external::audio_meta::extract(&path_for_task)
    })
    .await
    {
        Ok(Ok(meta)) => meta,
        Ok(Err(e)) => {
            tracing::debug!(track_id, file_hash, "Audio metadata extraction failed: {}", e);
            return None;
        }
        Err(e) => {
            tracing::warn!(track_id, "Metadata extraction task failed: {}", e);
            return None;
        }
    };

    let db = state.db.clone();
    let meta_clone = extracted.clone();
    if tokio::task::spawn_blocking(move || {
        db.upsert_track_metadata(track_id, &meta_clone)
    })
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

pub async fn delete_cd_track_audio(
    State(state): State<AppState>,
    Path((_cd_id, track_id)): Path<(i64, i64)>,
) -> Result<Json<String>, StatusCode> {
    let db = state.db.clone();
    let audio_dir = state.audio_dir.as_str();

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

pub async fn add_cd_author(
    State(state): State<AppState>,
    Path((cd_id, author_id)): Path<(i64, i64)>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .add_cd_author(cd_id, author_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_cd_author(
    State(state): State<AppState>,
    Path((cd_id, author_id)): Path<(i64, i64)>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .remove_cd_author(cd_id, author_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_cd_author_order(
    State(state): State<AppState>,
    Path((cd_id, author_id)): Path<(i64, i64)>,
    Json(req): Json<serde_json::Value>,
) -> Result<StatusCode, StatusCode> {
    let sort_order = req["sort_order"].as_i64().unwrap_or(0);
    state
        .db
        .update_cd_author_order(cd_id, author_id, sort_order)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
