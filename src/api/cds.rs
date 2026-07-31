use crate::AppState;
use crate::api::upload_chunks::{ChunkQuery, StoreResult};
use crate::db::{CdInfo, CdWithTracks, NewCd, NewTrack};
use crate::db_models::CdMetadata;
use crate::external;
use crate::external::audio_meta::TrackMetadata;
use axum::{
    Json,
    extract::{Path, Query, State},
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
    pub grand_series_id: Option<i64>,
    pub author_ids: Option<Vec<i64>>,
    pub tracks: Option<Vec<ManualCdTrackRequest>>,
    pub metadata: Option<ManualCdMetadataRequest>,
    pub manual: Option<bool>,
    pub musicbrainz_release_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ManualCdMetadataRequest {
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub isrc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ManualCdTrackRequest {
    pub disc_number: Option<i64>,
    pub track_number: i64,
    pub title: String,
    pub duration: Option<String>,
    #[serde(flatten)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

fn album_artist_from_metadata(db: &crate::db::Db, cd_id: i64) -> Option<String> {
    db.get_cd_album_tag_consensus(cd_id)
        .ok()
        .and_then(|tags| tags.album_artist)
}

async fn discogs_fallback(state: &AppState, jan: &str) -> Result<CdInfo, ApiError> {
    if state.discogs_token.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "CD not found for this JAN"})),
        ));
    }
    tracing::info!("Falling back to Discogs for JAN={}", jan);
    match external::lookup_cd_discogs(&state.client, jan, &state.discogs_token).await {
        Ok(Some(info)) => Ok(info),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "CD not found for this JAN"})),
        )),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("Discogs error: {}", e)})),
        )),
    }
}

pub async fn cd_register(
    State(state): State<AppState>,
    Json(req): Json<CdRegisterRequest>,
) -> Result<(StatusCode, Json<CdWithTracks>), ApiError> {
    let is_manual = req.manual.unwrap_or(false)
        || req
            .title
            .as_ref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false);

    if is_manual {
        let title = req.title.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Title is required for manual registration"})),
            )
        })?;
        let jan = req
            .jan
            .as_ref()
            .map(|j| j.replace('-', "").replace(' ', ""))
            .filter(|j| !j.is_empty());
        let publish_date = external::normalize_publish_date_input(req.publish_date.as_deref())
            .map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": error})),
                )
            })?;

        if let Some(ref j) = jan {
            if j.len() >= 8 {
                if let Ok(Some(existing)) = state.db.find_by_cd_jan(j) {
                    let cd_id = existing.id;
                    let tracks = state.db.list_tracks_for_cd(cd_id).unwrap_or_default();
                    let authors = state.db.get_cd_authors(cd_id).unwrap_or_default();
                    return Ok((
                        StatusCode::OK,
                        Json(CdWithTracks {
                            cd: existing,
                            album_artist: album_artist_from_metadata(&state.db, cd_id),
                            tracks,
                            authors,
                        }),
                    ));
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
            publish_date,
            cover_url: None,
            description: req.description,
            disc_count: req.disc_count,
            volume: req.volume,
            tracks: None,
            parent_book_id: req.parent_book_id,
            media_type: req.media_type.clone(),
            series_id: req.series_id,
        };

        let author_ids = req.author_ids.unwrap_or_default();
        let track_data: Vec<(NewTrack, Option<TrackMetadata>)> = req
            .tracks
            .unwrap_or_default()
            .into_iter()
            .map(|track| {
                let mut metadata = track.metadata;
                metadata.insert(
                    "title".to_string(),
                    serde_json::Value::String(track.title.clone()),
                );
                metadata.insert(
                    "track_number".to_string(),
                    serde_json::Value::Number(track.track_number.into()),
                );
                metadata.insert(
                    "disc_number".to_string(),
                    serde_json::Value::Number(track.disc_number.unwrap_or(1).into()),
                );
                (
                    NewTrack {
                        disc_number: track.disc_number,
                        track_number: track.track_number,
                        title: track.title,
                        duration: track.duration,
                    },
                    Some(TrackMetadata::from_json(&serde_json::Value::Object(
                        metadata,
                    ))),
                )
            })
            .collect();
        let album_metadata = req.metadata.map(|metadata| CdMetadata {
            cd_id: 0,
            year: metadata.year,
            genre: metadata.genre,
            composer: metadata.composer,
            isrc: metadata.isrc,
            ..CdMetadata::default()
        });

        let cd = state
            .db
            .insert_manual_cd(
                &new_cd,
                &author_ids,
                &track_data,
                album_metadata.as_ref(),
                req.grand_series_id,
            )
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;

        let cd_id = cd.id;
        let tracks = state.db.list_tracks_for_cd(cd_id).unwrap_or_default();
        let authors = state.db.get_cd_authors(cd_id).unwrap_or_default();
        return Ok((
            StatusCode::CREATED,
            Json(CdWithTracks {
                cd,
                album_artist: album_artist_from_metadata(&state.db, cd_id),
                tracks,
                authors,
            }),
        ));
    }

    let jan = req
        .jan
        .unwrap_or_default()
        .replace('-', "")
        .replace(' ', "");
    if jan.len() < 8 || jan.len() > 14 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid JAN length"})),
        ));
    }

    if let Ok(Some(existing)) = state.db.find_by_cd_jan(&jan) {
        let cd_id = existing.id;
        let tracks = state.db.list_tracks_for_cd(cd_id).unwrap_or_default();
        let authors = state.db.get_cd_authors(cd_id).unwrap_or_default();
        return Ok((
            StatusCode::OK,
            Json(CdWithTracks {
                cd: existing,
                album_artist: album_artist_from_metadata(&state.db, cd_id),
                tracks,
                authors,
            }),
        ));
    }

    let cd_info = if let Some(release_id) = req.musicbrainz_release_id.as_deref() {
        match external::lookup_cd_by_release_id(
            &state.client,
            release_id,
            &state.musicbrainz_contact,
        )
        .await
        {
            Ok(info) => Some(info),
            Err(e) => {
                return Err((
                    StatusCode::BAD_GATEWAY,
                    Json(
                        serde_json::json!({"error": format!("MusicBrainz release lookup failed: {}", e)}),
                    ),
                ));
            }
        }
    } else {
        let lookup = match external::lookup_cd(
            &state.client,
            &jan,
            &state.images_dir,
            &state.musicbrainz_contact,
        )
        .await
        {
            ok @ Ok(_) => ok,
            Err(e) => {
                tracing::warn!("MusicBrainz lookup failed: {}. Retrying...", e);
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                external::lookup_cd(
                    &state.client_ipv4,
                    &jan,
                    &state.images_dir,
                    &state.musicbrainz_contact,
                )
                .await
            }
        };

        match lookup {
            Ok(Some(info)) => Some(info),
            Ok(None) => {
                tracing::info!("MusicBrainz returned no results for JAN={}", jan);
                None
            }
            Err(e) => {
                tracing::warn!("MusicBrainz lookup error: {}", e);
                None
            }
        }
    };

    let cd_info = match cd_info {
        Some(info) => info,
        None => {
            let amazon_title =
                match external::lookup_amazon_title_for_jan(&state.client, &jan).await {
                    Some(title) => Some(title),
                    None => {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        external::lookup_amazon_title_for_jan(&state.client_ipv4, &jan).await
                    }
                };

            if let Some(title) = amazon_title {
                match external::search_cd_candidates_by_title(
                    &state.client,
                    &title,
                    &state.musicbrainz_contact,
                )
                .await
                {
                    Ok(candidates) if candidates.len() > 1 => {
                        return Err((
                            StatusCode::MULTIPLE_CHOICES,
                            Json(serde_json::json!({
                                "code": "musicbrainz_candidates",
                                "error": "MusicBrainzの候補を選択してください",
                                "jan": jan,
                                "amazon_title": title,
                                "candidates": candidates,
                            })),
                        ));
                    }
                    Ok(mut candidates) if candidates.len() == 1 => {
                        let candidate = candidates.remove(0);
                        match external::lookup_cd_by_release_id(
                            &state.client,
                            &candidate.id,
                            &state.musicbrainz_contact,
                        )
                        .await
                        {
                            Ok(info) => info,
                            Err(e) => {
                                tracing::warn!(
                                    release_id = %candidate.id,
                                    "MusicBrainz candidate lookup failed: {}",
                                    e
                                );
                                return Err((
                                    StatusCode::BAD_GATEWAY,
                                    Json(
                                        serde_json::json!({"error": format!("MusicBrainz候補の取得に失敗しました: {}", e)}),
                                    ),
                                ));
                            }
                        }
                    }
                    Ok(_) => {
                        tracing::info!(amazon_title = %title, "MusicBrainz title search returned no candidates");
                        match discogs_fallback(&state, &jan).await {
                            Ok(info) => info,
                            Err(error) => return Err(error),
                        }
                    }
                    Err(e) => {
                        tracing::warn!(amazon_title = %title, "MusicBrainz title search failed: {}", e);
                        match discogs_fallback(&state, &jan).await {
                            Ok(info) => info,
                            Err(error) => return Err(error),
                        }
                    }
                }
            } else {
                tracing::info!("Amazon title lookup returned no result for JAN={}", jan);
                match discogs_fallback(&state, &jan).await {
                    Ok(info) => info,
                    Err(error) => return Err(error),
                }
            }
        }
    };

    let amazon_cover =
        match external::lookup_amazon_cover_for_jan(&state.client, &jan, &state.images_dir).await {
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

    let publish_date = external::normalize_publish_date_input(cd_info.publish_date.as_deref())
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": error})),
            )
        })?;
    let new_cd = NewCd {
        jan: Some(jan),
        title: cd_info.title,
        artist: cd_info.artist,
        publisher: cd_info.publisher,
        label: cd_info.label,
        catalog_number: cd_info.catalog_number,
        publish_date,
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

    let cd_id = cd.id;
    let tracks = state.db.list_tracks_for_cd(cd_id).unwrap_or_default();
    Ok((
        StatusCode::CREATED,
        Json(CdWithTracks {
            cd,
            album_artist: album_artist_from_metadata(&state.db, cd_id),
            tracks,
            authors: vec![],
        }),
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
            let album_artist = album_artist_from_metadata(&db, cd.id);
            result.push(CdWithTracks {
                cd,
                album_artist,
                tracks,
                authors,
            });
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
        db.search_tracks_by_metadata(q.artist.as_deref(), q.album.as_deref(), q.year, limit)
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
    if state.db.delete_cd(id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "CD not found"})),
        ))
    }
}

/// Present/absent helper for string fields: key present with a string => Some(string);
/// key present with null (or a non-string) => None (clear); key absent => preserve existing.
fn present_str(body: &serde_json::Value, key: &str, existing: Option<&str>) -> Option<String> {
    match body.get(key) {
        Some(v) => v.as_str().map(|s| s.to_string()),
        None => existing.map(|s| s.to_string()),
    }
}

/// Present/absent helper for integer fields: key present => its i64 (null/non-int => None, clear);
/// key absent => preserve existing.
fn present_i64(body: &serde_json::Value, key: &str, existing: Option<i64>) -> Option<i64> {
    match body.get(key) {
        Some(v) => v.as_i64(),
        None => existing,
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

    let jan = present_str(&body, "jan", existing.jan.as_deref());
    let artist = present_str(&body, "artist", existing.artist.as_deref());
    let publisher = present_str(&body, "publisher", existing.publisher.as_deref());
    let label = present_str(&body, "label", existing.label.as_deref());
    let catalog_number = present_str(&body, "catalog_number", existing.catalog_number.as_deref());
    let publish_date = match body.get("publish_date") {
        Some(value) if value.is_null() => None,
        Some(value) => {
            let raw = value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "publish_date must be a string"})),
                )
            })?;
            external::normalize_publish_date_input(Some(raw)).map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": error})),
                )
            })?
        }
        None => existing.publish_date.clone(),
    };
    let description = present_str(&body, "description", existing.description.as_deref());
    let disc_count = present_i64(&body, "disc_count", existing.disc_count);
    let volume = present_str(&body, "volume", existing.volume.as_deref());
    let parent_book_id = present_i64(&body, "parent_book_id", existing.parent_book_id);
    let media_type = present_str(&body, "media_type", existing.media_type.as_deref());
    let series_id = present_i64(&body, "series_id", existing.series_id);
    let metadata = match body.get("metadata") {
        None => None,
        Some(value) if value.is_object() => Some(CdMetadata::from_json(id, value)),
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "metadata must be an object"})),
            ));
        }
    };

    state
        .db
        .update_cd_with_metadata(
            id,
            jan.as_deref(),
            title.trim(),
            artist.as_deref(),
            publisher.as_deref(),
            label.as_deref(),
            catalog_number.as_deref(),
            publish_date.as_deref(),
            description.as_deref(),
            disc_count,
            volume.as_deref(),
            parent_book_id,
            media_type.as_deref(),
            series_id,
            metadata.as_ref(),
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
    // The track modal only sends the user-editable fields; preserve everything the
    // audio extraction produced (artist/album/cover/ReplayGain/file info/...).
    let db = state.db.clone();
    let existing = match tokio::task::spawn_blocking(move || db.get_track_metadata(track_id)).await
    {
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
        tracing::error!(track_id, "track_metadata task failed: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    if let Err(e) = join_result.as_ref().unwrap().as_ref() {
        tracing::error!(track_id, "track_metadata upsert failed: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
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
    // The album-info edit form only sends {year,genre,composer,isrc}; preserve the
    // extracted cover art and album ReplayGain that the request never carries.
    let db = state.db.clone();
    let existing = match tokio::task::spawn_blocking(move || db.get_cd_metadata(cd_id)).await {
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
    let mut merged = CdMetadata::from_json(cd_id, &body);
    if let Some(ex) = existing {
        merged.cover_mime = ex.cover_mime;
        merged.cover_data = ex.cover_data;
        merged.replay_gain_album_gain_db = ex.replay_gain_album_gain_db;
        merged.replay_gain_album_peak = ex.replay_gain_album_peak;
    }
    let db = state.db.clone();
    let join_result =
        tokio::task::spawn_blocking(move || db.upsert_cd_metadata(cd_id, &merged)).await;
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
    Path((cd_id, track_id)): Path<(i64, i64)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<String>, StatusCode> {
    if let Some(swap_track_id) = body.get("swap_track_id").and_then(|v| v.as_i64()) {
        let swapped = tokio::task::spawn_blocking({
            let db = state.db.clone();
            move || db.swap_track_positions(cd_id, track_id, swap_track_id)
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if !swapped {
            return Err(StatusCode::BAD_REQUEST);
        }
        return Ok(Json("ok".into()));
    }

    let title = body
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let duration = body
        .get("duration")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
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
        let tn = body
            .get("track_number")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);
        let db2 = state.db.clone();
        tokio::task::spawn_blocking(move || db2.update_track_position(track_id, disc, tn))
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
    Path((cd_id, track_id)): Path<(i64, i64)>,
    Query(chunk): Query<ChunkQuery>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let track = state
        .db
        .find_track_by_id(track_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if track.cd_id != Some(cd_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let audio_dir = state.audio_dir.as_str();
    let chunk_info = chunk.validate().map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut file_hash: Option<String> = None;
    let mut file_name: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let name = field.file_name().unwrap_or("unknown").to_string();
        let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
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
            .map_err(|_| StatusCode::BAD_REQUEST)?
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
                    let result =
                        external::save_uploaded_audio_path(&path, &name, audio_dir, audio_max);
                    let _ = std::fs::remove_dir_all(cleanup_dir);
                    result
                }
            }
            .map_err(|e| {
                tracing::warn!(track_id, cd_id, "Audio save failed: {}", e);
                StatusCode::BAD_REQUEST
            })?,
            None => {
                external::save_uploaded_audio(&data, &name, audio_dir, audio_max).map_err(|e| {
                    tracing::warn!(track_id, cd_id, "Audio save failed: {}", e);
                    StatusCode::BAD_REQUEST
                })?
            }
        };

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

    let metadata = extract_and_save_metadata(&state, track_id, cd_id, &hash).await;

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

    let extracted =
        match tokio::task::spawn_blocking(move || external::audio_meta::extract(&path_for_task))
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
    let track_meta = extracted.clone();
    let track_save =
        tokio::task::spawn_blocking(move || db.upsert_track_metadata(track_id, &track_meta)).await;

    if let Err(e) = track_save.as_ref() {
        tracing::warn!(track_id, "Track metadata save task failed: {}", e);
    } else if let Err(e) = track_save.as_ref().unwrap().as_ref() {
        tracing::warn!(track_id, "Track metadata upsert failed: {}", e);
    }

    let db = state.db.clone();
    let cd_meta = extracted.clone().into_cd_metadata(cd_id);
    let cd_save = tokio::task::spawn_blocking(move || db.upsert_cd_metadata(cd_id, &cd_meta)).await;

    if let Err(e) = cd_save.as_ref() {
        tracing::warn!(cd_id, "CD metadata save task failed: {}", e);
    } else if let Err(e) = cd_save.as_ref().unwrap().as_ref() {
        tracing::warn!(cd_id, "CD metadata upsert failed: {}", e);
    }

    synchronize_cd_core_metadata(state, track_id, cd_id, &extracted).await;

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

async fn synchronize_cd_core_metadata(
    state: &AppState,
    track_id: i64,
    cd_id: i64,
    extracted: &crate::external::audio_meta::TrackMetadata,
) {
    let db = state.db.clone();
    let extracted = extracted.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
        let Some(cd) = db.find_cd_by_id(cd_id)? else {
            return Ok(());
        };

        let non_empty = non_empty_audio_tag;
        let positive = |value: Option<i64>| value.filter(|value| *value > 0);

        let title = non_empty(extracted.album.as_ref())
            .unwrap_or(cd.title.as_str())
            .to_string();
        let artist = non_empty(extracted.album_artist.as_ref())
            .or_else(|| non_empty(extracted.artist.as_ref()))
            .map(str::to_string)
            .or(cd.artist.clone());
        let publisher = non_empty(extracted.publisher.as_ref())
            .map(str::to_string)
            .or(cd.publisher.clone());
        let label = non_empty(extracted.label.as_ref())
            .map(str::to_string)
            .or(cd.label.clone());
        let publish_date = positive(extracted.year)
            .map(|year| year.to_string())
            .or(cd.publish_date.clone());
        let disc_count = positive(extracted.disc_total)
            .or_else(|| {
                positive(extracted.disc_number).map(|number| cd.disc_count.unwrap_or(1).max(number))
            })
            .or(cd.disc_count);

        db.update_cd(
            cd_id,
            cd.jan.as_deref(),
            &title,
            artist.as_deref(),
            publisher.as_deref(),
            label.as_deref(),
            cd.catalog_number.as_deref(),
            publish_date.as_deref(),
            cd.description.as_deref(),
            disc_count,
            cd.volume.as_deref(),
            cd.parent_book_id,
            cd.media_type.as_deref(),
            cd.series_id,
        )?;

        db.update_track(track_id, non_empty(extracted.title.as_ref()), None)?;
        if let (Some(disc_number), Some(track_number)) = (
            positive(extracted.disc_number),
            positive(extracted.track_number),
        ) {
            db.update_track_position(track_id, disc_number, track_number)?;
        }
        Ok(())
    })
    .await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(track_id, cd_id, "Core audio metadata sync failed: {}", e),
        Err(e) => tracing::warn!(
            track_id,
            cd_id,
            "Core audio metadata sync task failed: {}",
            e
        ),
    }
}

fn non_empty_audio_tag(value: Option<&String>) -> Option<&str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

pub async fn delete_cd_track_audio(
    State(state): State<AppState>,
    Path((cd_id, track_id)): Path<(i64, i64)>,
) -> Result<Json<String>, StatusCode> {
    let db = state.db.clone();
    let audio_dir = state.audio_dir.as_str();

    let track = tokio::task::spawn_blocking(move || db.find_track_by_id(track_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if track.as_ref().and_then(|track| track.cd_id) != Some(cd_id) {
        return Err(StatusCode::NOT_FOUND);
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
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json("ok".into()))
}

pub async fn upload_cd_cover(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(chunk): Query<ChunkQuery>,
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
    let chunk_info = chunk.validate().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid upload chunk parameters"})),
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
            let bytes = match chunk_info.as_ref() {
                Some(info) => match crate::api::upload_chunks::store_chunk(
                    state.uploads_dir.as_str(),
                    "cover",
                    info.clone(),
                    &bytes,
                )
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": "Invalid upload chunk"})),
                    )
                })? {
                    StoreResult::Partial { part, total_parts } => {
                        return Ok(Json(serde_json::json!({
                            "chunked": true,
                            "complete": false,
                            "part": part,
                            "total_parts": total_parts,
                        })));
                    }
                    StoreResult::Complete { path, cleanup_dir } => {
                        let result = std::fs::read(&path).map_err(|e| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": e.to_string()})),
                            )
                        });
                        let _ = std::fs::remove_dir_all(cleanup_dir);
                        result?
                    }
                },
                None => bytes,
            };
            let cover_max = crate::api::upload_limit_bytes(
                &state,
                crate::api::KEY_UPLOAD_COVER_MB,
                crate::api::COVER_MAX_BYTES,
            );
            if bytes.len() > cover_max {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        serde_json::json!({"error": format!("File too large (max {}MB)", cover_max / 1024 / 1024)}),
                    ),
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

/// CD 配下の track_metadata から集約したアルバムレベルタグ(参考値)を返す。読取専用。
pub async fn get_cd_album_tags(
    State(state): State<AppState>,
    Path(cd_id): Path<i64>,
) -> Result<Json<crate::db_models::AlbumTagConsensus>, StatusCode> {
    let db = state.db.clone();
    let consensus = tokio::task::spawn_blocking(move || db.get_cd_album_tag_consensus(cd_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(consensus))
}

#[derive(serde::Deserialize)]
pub struct AddCdAuthorsFromNamesRequest {
    pub names: Vec<String>,
}

/// 名前リストから作者を確保(同名は既存を再利用)し、CD のアルバムアーティストとして紐付ける。
/// 空欄へタグ由来のアルバムアーティストを登録する用途。冪等。
pub async fn add_cd_authors_from_names(
    State(state): State<AppState>,
    Path(cd_id): Path<i64>,
    Json(req): Json<AddCdAuthorsFromNamesRequest>,
) -> Result<StatusCode, StatusCode> {
    let names = req
        .names
        .into_iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>();
    if names.is_empty() {
        return Ok(StatusCode::NO_CONTENT);
    }
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || -> Result<(), rusqlite::Error> {
        let ids = db.ensure_authors_for_names(&names)?;
        for id in ids {
            db.add_cd_author(cd_id, id)?;
        }
        Ok(())
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
