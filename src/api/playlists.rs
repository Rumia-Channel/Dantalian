use crate::AppState;
use crate::db::PlaylistWithTracks;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

use super::books::ApiError;

#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
    pub description: Option<String>,
    pub cover_cd_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlaylistRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub cover_cd_id: Option<Option<i64>>,
    pub track_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
pub struct SetPlaylistTracksRequest {
    pub track_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AddPlaylistTrackRequest {
    pub track_id: i64,
}

fn error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(serde_json::json!({ "error": message.into() })))
}

async fn load_playlist(state: &AppState, id: i64) -> Result<PlaylistWithTracks, ApiError> {
    state
        .db
        .find_playlist_by_id(id)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "Playlist not found"))
}

fn validate_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "Playlist name is required"));
    }
    if name.chars().count() > 200 {
        return Err(error(StatusCode::BAD_REQUEST, "Playlist name is too long"));
    }
    Ok(name.to_string())
}

fn validate_cover(state: &AppState, cover_cd_id: Option<i64>) -> Result<(), ApiError> {
    if let Some(cd_id) = cover_cd_id {
        let exists = state
            .db
            .find_cd_by_id(cd_id)
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .is_some();
        if !exists {
            return Err(error(StatusCode::BAD_REQUEST, "Cover CD not found"));
        }
    }
    Ok(())
}

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<PlaylistWithTracks>>, StatusCode> {
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || db.list_playlists())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(result))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<PlaylistWithTracks>, ApiError> {
    Ok(Json(load_playlist(&state, id).await?))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreatePlaylistRequest>,
) -> Result<(StatusCode, Json<PlaylistWithTracks>), ApiError> {
    let name = validate_name(&body.name)?;
    validate_cover(&state, body.cover_cd_id)?;
    let playlist = state
        .db
        .insert_playlist(&name, body.description.as_deref(), body.cover_cd_id)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let result = load_playlist(&state, playlist.id).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdatePlaylistRequest>,
) -> Result<Json<PlaylistWithTracks>, ApiError> {
    let existing = load_playlist(&state, id).await?;
    let name = match body.name {
        Some(name) => validate_name(&name)?,
        None => existing.playlist.name,
    };
    let description = body.description.unwrap_or(existing.playlist.description);
    let cover_cd_id = body.cover_cd_id.unwrap_or(existing.playlist.cover_cd_id);
    validate_cover(&state, cover_cd_id)?;
    let updated = match body.track_ids.as_deref() {
        Some(track_ids) => state.db.update_playlist_with_tracks(
            id,
            &name,
            description.as_deref(),
            cover_cd_id,
            track_ids,
        ),
        None => state
            .db
            .update_playlist(id, &name, description.as_deref(), cover_cd_id),
    }
    .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !updated {
        return Err(error(StatusCode::NOT_FOUND, "Playlist not found"));
    }
    Ok(Json(load_playlist(&state, id).await?))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .db
        .delete_playlist(id)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(error(StatusCode::NOT_FOUND, "Playlist not found"))
    }
}

pub async fn set_tracks(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<SetPlaylistTracksRequest>,
) -> Result<Json<PlaylistWithTracks>, ApiError> {
    load_playlist(&state, id).await?;
    let updated = state
        .db
        .set_playlist_tracks(id, &body.track_ids)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !updated {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Playlist contains an invalid audio track",
        ));
    }
    Ok(Json(load_playlist(&state, id).await?))
}

pub async fn add_track(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<AddPlaylistTrackRequest>,
) -> Result<Json<PlaylistWithTracks>, ApiError> {
    load_playlist(&state, id).await?;
    let added = state
        .db
        .add_playlist_track(id, body.track_id)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !added {
        return Err(error(StatusCode::BAD_REQUEST, "Audio track not found"));
    }
    Ok(Json(load_playlist(&state, id).await?))
}

pub async fn remove_track(
    State(state): State<AppState>,
    Path((id, track_id)): Path<(i64, i64)>,
) -> Result<Json<PlaylistWithTracks>, ApiError> {
    load_playlist(&state, id).await?;
    let removed = state
        .db
        .remove_playlist_track(id, track_id)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !removed {
        return Err(error(StatusCode::NOT_FOUND, "Playlist track not found"));
    }
    Ok(Json(load_playlist(&state, id).await?))
}
