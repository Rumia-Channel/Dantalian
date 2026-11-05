use super::error::{ApiError, error_response};
use crate::{
    AppState,
    application::error::AppError,
    db::{BookWithAuthors, CdWithTracks, NewBook, NewCd, NewTrack},
    dlsite::{DlsiteMediaType, DlsiteRegisterRequest, DlsiteWork},
};
use axum::{Json, extract::State, http::StatusCode};

fn database_error(error: rusqlite::Error) -> ApiError {
    error_response(AppError::Database(error.to_string()))
}

async fn lookup(state: &AppState, id: &str) -> Result<DlsiteWork, ApiError> {
    crate::external::dlsite::lookup(&state.client, id, &state.images_dir)
        .await
        .map_err(|error| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error":error})),
            )
        })?
        .ok_or_else(|| error_response(AppError::NotFound))
}

pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<DlsiteRegisterRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let id = crate::dlsite::normalize_product_id(Some(&request.dlsite_id))
        .map_err(|message| error_response(AppError::Validation(message.to_owned())))?
        .ok_or_else(|| error_response(AppError::Validation("DLsite ID is required".to_owned())))?;
    if request.media_type == DlsiteMediaType::Book {
        let (book, status, source) =
            match state.db.find_by_dlsite_id(&id).map_err(database_error)? {
                Some(book) => (book, StatusCode::OK, "cache"),
                None => {
                    let work = lookup(&state, &id).await?;
                    let book = state
                        .db
                        .insert_book(&NewBook {
                            dlsite_id: Some(work.dlsite_id),
                            title: work.title,
                            publisher: work.publisher,
                            publish_date: work.publish_date,
                            description: work.description,
                            series_title: work.series_title,
                            price: work.price,
                            extent: work.extent,
                            cover_url: work.cover_url,
                            media_type: Some("book".to_owned()),
                            ..NewBook::default()
                        })
                        .map_err(database_error)?;
                    for name in work.authors {
                        let author_id = state
                            .db
                            .insert_author(None, &name, None)
                            .map_err(database_error)?;
                        state
                            .db
                            .add_book_author(book.id, author_id)
                            .map_err(database_error)?;
                    }
                    (book, StatusCode::CREATED, "dlsite")
                }
            };
        let authors = state.db.get_book_authors(book.id).map_err(database_error)?;
        let (copies_count, lent_count) = state
            .db
            .get_book_copy_counts(book.id)
            .map_err(database_error)?;
        return Ok((
            status,
            Json(serde_json::json!({
                "book": BookWithAuthors { book, authors, copies_count, lent_count },
                "source": source,
            })),
        ));
    }
    let kind = request.media_type.as_str();
    let (cd, status, source) = match state.db.find_cd_by_dlsite_id(&id).map_err(database_error)? {
        Some(cd) => {
            if cd.media_type.as_deref() != Some(kind) {
                return Err(error_response(AppError::Conflict(
                    "DLsite work is already registered with another audio type".to_owned(),
                )));
            }
            (cd, StatusCode::OK, "cache")
        }
        None => {
            let work = lookup(&state, &id).await?;
            let author_ids = work
                .authors
                .iter()
                .map(|name| state.db.insert_author(None, name, None))
                .collect::<Result<Vec<_>, _>>()
                .map_err(database_error)?;
            let tracks = work
                .tracks
                .into_iter()
                .enumerate()
                .map(|(index, track)| {
                    (
                        NewTrack {
                            disc_number: Some(1),
                            track_number: index as i64 + 1,
                            title: track.title,
                            duration: track.duration,
                        },
                        None,
                    )
                })
                .collect::<Vec<_>>();
            let cd = state
                .db
                .insert_manual_cd(
                    &NewCd {
                        dlsite_id: Some(work.dlsite_id),
                        title: work.title,
                        artist: (!work.voice_actors.is_empty())
                            .then(|| work.voice_actors.join(" / ")),
                        publisher: work.publisher,
                        publish_date: work.publish_date,
                        description: work.description,
                        cover_url: work.cover_url,
                        media_type: Some(kind.to_owned()),
                        ..NewCd::default()
                    },
                    &author_ids,
                    &tracks,
                    None,
                    None,
                )
                .map_err(database_error)?;
            (cd, StatusCode::CREATED, "dlsite")
        }
    };
    let tracks = state.db.list_tracks_for_cd(cd.id).map_err(database_error)?;
    let authors = state.db.get_cd_authors(cd.id).map_err(database_error)?;
    let (track_artist, album_artist) = super::cds::cd_tag_artists_from_metadata(&state.db, cd.id);
    Ok((
        status,
        Json(serde_json::json!({
            "cd": CdWithTracks { cd, track_artist, album_artist, tracks, authors },
            "source": source,
        })),
    ))
}
