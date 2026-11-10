use crate::{AppState, db::Track, domain::media::is_valid_track_position};
use axum::http::StatusCode;

pub(crate) enum TrackParent {
    Book(i64),
    Cd(i64),
}

/// Parsed fields of a track update request.
///
/// Every field is optional: `None` means "not supplied" and the stored value
/// has to be kept as is.
#[derive(Debug, Default, serde::Deserialize, PartialEq)]
pub(crate) struct TrackPatch {
    pub title: Option<String>,
    pub duration: Option<String>,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
}

impl TrackPatch {
    pub(crate) fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.duration.is_none()
            && self.disc_number.is_none()
            && self.track_number.is_none()
    }
}

/// Reads the track fields of a request body.
///
/// Absent (or null) text and number fields mean "keep the stored value".
/// A supplied number must be a positive integer inside the range the D1 backed
/// Worker can store; anything else is rejected so a bad request never writes
/// a silently changed position.
pub(crate) fn parse_track_patch(body: serde_json::Value) -> Result<TrackPatch, StatusCode> {
    let patch: TrackPatch = serde_json::from_value(body).map_err(|_| StatusCode::BAD_REQUEST)?;
    if patch
        .disc_number
        .is_some_and(|number| !is_valid_track_position(number))
        || patch
            .track_number
            .is_some_and(|number| !is_valid_track_position(number))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(patch)
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

#[cfg(test)]
mod tests {
    use super::{TrackPatch, parse_track_patch};
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn keeps_unsupplied_fields() {
        let patch = parse_track_patch(json!({"disc_number": 2})).expect("patch");
        assert_eq!(
            patch,
            TrackPatch {
                disc_number: Some(2),
                ..TrackPatch::default()
            }
        );
        assert!(!patch.is_empty());

        let patch = parse_track_patch(json!({"track_number": 7})).expect("patch");
        assert_eq!(
            patch,
            TrackPatch {
                track_number: Some(7),
                ..TrackPatch::default()
            }
        );

        let patch = parse_track_patch(json!({})).expect("patch");
        assert!(patch.is_empty());
    }

    #[test]
    fn treats_null_fields_as_unsupplied() {
        let patch =
            parse_track_patch(json!({"disc_number": null, "track_number": null})).expect("patch");
        assert_eq!(patch, TrackPatch::default());
    }

    #[test]
    fn rejects_invalid_positions() {
        for (case, body) in [
            json!({"disc_number": 0}),
            json!({"disc_number": -1}),
            json!({"track_number": 0}),
            json!({"track_number": 2_147_483_648_i64}),
            json!({"disc_number": "2"}),
            json!({"track_number": 1.5}),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                parse_track_patch(body).err(),
                Some(StatusCode::BAD_REQUEST),
                "invalid position case {case}"
            );
        }
    }
}
