use crate::{AppState, db::Track};
use axum::http::StatusCode;

pub(crate) enum TrackParent {
    Book(i64),
    Cd(i64),
}

pub(crate) async fn require_track(
    state: &AppState,
    parent: TrackParent,
    track_id: i64,
) -> Result<Track, StatusCode> {
    let parent_id = match parent {
        TrackParent::Book(id) | TrackParent::Cd(id) => id,
    };
    if parent_id <= 0 || track_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let db = state.db.clone();
    let track = tokio::task::spawn_blocking(move || db.find_track_by_id(track_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let belongs = match parent {
        TrackParent::Book(id) => track.book_id == id && track.cd_id.is_none(),
        TrackParent::Cd(id) => track.cd_id == Some(id),
    };
    if !belongs {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(track)
}

pub(crate) async fn patch_metadata(
    state: &AppState,
    parent: TrackParent,
    track_id: i64,
    body: serde_json::Value,
) -> Result<axum::Json<String>, StatusCode> {
    require_track(state, parent, track_id).await?;
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || db.patch_track_metadata(track_id, body))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|error| crate::api::error::status_for_error(&error))?;
    Ok(axum::Json("ok".into()))
}
