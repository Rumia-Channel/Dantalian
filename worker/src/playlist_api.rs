use serde::{Deserialize, Serialize};
use worker::{D1Type, Request, Response, Result, RouteContext};

use crate::error::{bad_request, error_response, parse_id, parse_json};
use dantalian::application::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
struct PlaylistRow {
    id: i64,
    name: String,
    description: Option<String>,
    cover_cd_id: Option<i64>,
    cover_url: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateRequest {
    name: String,
    description: Option<String>,
    cover_cd_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UpdateRequest {
    name: Option<String>,
    description: Option<Option<String>>,
    cover_cd_id: Option<Option<i64>>,
    track_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
struct TrackRequest {
    track_id: i64,
}
#[derive(Debug, Deserialize)]
struct SetTracksRequest {
    track_ids: Vec<i64>,
}

fn db_error(error: worker::Error) -> worker::Error {
    error
}
fn id_type(id: i64, label: &str) -> std::result::Result<D1Type<'static>, AppError> {
    let value = i32::try_from(id).map_err(|_| AppError::Validation(format!("invalid {label}")))?;
    if value <= 0 {
        return Err(AppError::Validation(format!("invalid {label}")));
    }
    Ok(D1Type::Integer(value))
}
fn name(value: String) -> std::result::Result<String, AppError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::Validation("playlist name is required".into()));
    }
    if value.chars().count() > 200 {
        return Err(AppError::Validation("playlist name is too long".into()));
    }
    Ok(value)
}

async fn load(db: &worker::D1Database, raw_id: i64) -> Result<Option<serde_json::Value>> {
    let id =
        id_type(raw_id, "playlist id").map_err(|error| worker::Error::from(error.to_string()))?;
    let Some(playlist) = db.prepare("SELECT id,name,description,cover_cd_id,cover_url,created_at,updated_at FROM playlists WHERE id = ?").bind_refs(&id).map_err(db_error)?.first::<PlaylistRow>(None).await.map_err(db_error)? else { return Ok(None); };
    let tracks = db.prepare("SELECT pt.position, t.id, t.book_id, t.cd_id, t.disc_number, t.track_number, t.title, t.duration, t.file_hash, t.file_name, c.id AS cd_id_value, c.title AS cd_title, c.artist AS cd_artist, c.cover_url AS cd_cover_url FROM playlist_tracks pt JOIN tracks t ON t.id = pt.track_id LEFT JOIN cds c ON c.id = t.cd_id WHERE pt.playlist_id = ? ORDER BY pt.position, pt.track_id").bind_refs(&id).map_err(db_error)?.all().await.map_err(db_error)?.results::<serde_json::Value>().map_err(db_error)?;
    let mut value =
        serde_json::to_value(playlist).map_err(|error| worker::Error::from(error.to_string()))?;
    if let Some(object) = value.as_object_mut() {
        object.insert("tracks".into(), serde_json::Value::Array(tracks));
    }
    Ok(Some(value))
}

async fn ensure_track(db: &worker::D1Database, track_id: &D1Type<'_>) -> Result<bool> {
    Ok(db
        .prepare(
            "SELECT id FROM tracks
             WHERE id = ? AND cd_id IS NOT NULL AND file_hash IS NOT NULL AND file_hash <> ''",
        )
        .bind_refs(track_id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .is_some())
}

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DB")?;
    let rows = db
        .prepare("SELECT id FROM playlists ORDER BY name,id")
        .all()
        .await
        .map_err(db_error)?
        .results::<serde_json::Value>()
        .map_err(db_error)?;
    let mut values = Vec::new();
    for value in rows {
        if let Some(id) = value.get("id").and_then(|value| value.as_i64()) {
            if let Some(playlist) = load(&db, id).await? {
                values.push(playlist);
            }
        }
    }
    Response::from_json(&values)
}

pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    match load(&ctx.d1("DB")?, raw).await? {
        Some(value) => Response::from_json(&value),
        None => error_response(AppError::NotFound),
    }
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body = match parse_json::<CreateRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let name = match name(body.name) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let cover = match body.cover_cd_id {
        Some(value) => Some(
            id_type(value, "cover_cd_id")
                .map_err(|error| worker::Error::from(error.to_string()))?,
        ),
        None => None,
    };
    let db = ctx.d1("DB")?;
    if let Some(cover) = &cover {
        if db
            .prepare("SELECT id FROM cds WHERE id = ?")
            .bind_refs(cover)
            .map_err(db_error)?
            .first::<serde_json::Value>(None)
            .await
            .map_err(db_error)?
            .is_none()
        {
            return Ok(bad_request("cover CD not found"));
        }
    }
    let row = db
        .prepare(
            "INSERT INTO playlists (name,description,cover_cd_id,created_at,updated_at)
             VALUES (?,?,?,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP) RETURNING id",
        )
        .bind_refs([
            &D1Type::Text(&name),
            &body
                .description
                .as_deref()
                .map(D1Type::Text)
                .unwrap_or(D1Type::Null),
            cover.as_ref().unwrap_or(&D1Type::Null),
        ])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    let Some(id) = row.and_then(|value| value.get("id").and_then(|value| value.as_i64())) else {
        return error_response(AppError::Internal("playlist insert returned no row".into()));
    };
    let value = load(&db, id)
        .await?
        .ok_or_else(|| worker::Error::from("playlist insert returned no row"))?;
    Response::from_json(&value).map(|response| response.with_status(201))
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, "playlist id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match parse_json::<UpdateRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let Some(current) = load(&db, raw).await? else {
        return error_response(AppError::NotFound);
    };
    let current_name = current
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let playlist_name = match body.name {
        Some(value) => match name(value) {
            Ok(value) => value,
            Err(error) => return error_response(error),
        },
        None => current_name,
    };
    let description = body.description.unwrap_or_else(|| {
        current
            .get("description")
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    });
    let cover_cd_id = body
        .cover_cd_id
        .unwrap_or_else(|| current.get("cover_cd_id").and_then(|value| value.as_i64()));
    let cover = match cover_cd_id {
        Some(value) => {
            let cover = id_type(value, "cover_cd_id")
                .map_err(|error| worker::Error::from(error.to_string()))?;
            if db
                .prepare("SELECT id FROM cds WHERE id = ?")
                .bind_refs(&cover)
                .map_err(db_error)?
                .first::<serde_json::Value>(None)
                .await
                .map_err(db_error)?
                .is_none()
            {
                return Ok(bad_request("cover CD not found"));
            }
            Some(cover)
        }
        None => None,
    };
    db.prepare(
        "UPDATE playlists
         SET name = ?, description = ?, cover_cd_id = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind_refs([
        &D1Type::Text(&playlist_name),
        &description
            .as_deref()
            .map(D1Type::Text)
            .unwrap_or(D1Type::Null),
        cover.as_ref().unwrap_or(&D1Type::Null),
        &id,
    ])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    if let Some(track_ids) = body.track_ids {
        if let Err(error) = set_track_ids(&db, raw, &track_ids).await {
            return Ok(bad_request(error.to_string()));
        }
    }
    Response::from_json(&load(&db, raw).await?.unwrap())
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, "playlist id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let result = ctx
        .d1("DB")?
        .prepare("DELETE FROM playlists WHERE id = ?")
        .bind_refs(&id)
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    if result
        .meta()
        .map_err(db_error)?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        == 0
    {
        return error_response(AppError::NotFound);
    }
    Ok(Response::empty()?.with_status(204))
}

async fn set_track_ids(db: &worker::D1Database, playlist_id: i64, track_ids: &[i64]) -> Result<()> {
    let playlist = id_type(playlist_id, "playlist id")
        .map_err(|error| worker::Error::from(error.to_string()))?;
    let mut tracks = Vec::with_capacity(track_ids.len());
    for raw_track in track_ids {
        let track = id_type(*raw_track, "track id")
            .map_err(|error| worker::Error::from(error.to_string()))?;
        if !ensure_track(db, &track).await? {
            return Err(worker::Error::from(
                "playlist contains an invalid audio track",
            ));
        }
        tracks.push(track);
    }
    db.prepare("DELETE FROM playlist_tracks WHERE playlist_id = ?")
        .bind_refs(&playlist)
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    for (position, track) in tracks.iter().enumerate() {
        db.prepare("INSERT INTO playlist_tracks (playlist_id,track_id,position) VALUES (?,?,?)")
            .bind_refs([
                &playlist,
                track,
                &D1Type::Integer(i32::try_from(position).unwrap_or(i32::MAX)),
            ])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
    }
    Ok(())
}

pub async fn set_tracks(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let body = match parse_json::<SetTracksRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    if load(&db, raw).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    if let Err(error) = set_track_ids(&db, raw, &body.track_ids).await {
        return Ok(bad_request(error.to_string()));
    }
    Response::from_json(&load(&db, raw).await?.unwrap())
}

pub async fn add_track(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let body = match parse_json::<TrackRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    if load(&db, raw).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    let existing = db.prepare("SELECT COALESCE(MAX(position), -1) + 1 AS position FROM playlist_tracks WHERE playlist_id = ?").bind_refs(&id_type(raw,"playlist id").map_err(|error|worker::Error::from(error.to_string()))?).map_err(db_error)?.first::<serde_json::Value>(None).await.map_err(db_error)?;
    let position = existing
        .and_then(|value| value.get("position").and_then(|value| value.as_i64()))
        .unwrap_or(0);
    let track = match id_type(body.track_id, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    if !ensure_track(&db, &track).await? {
        return Ok(bad_request("audio track not found"));
    }
    let playlist =
        id_type(raw, "playlist id").map_err(|error| worker::Error::from(error.to_string()))?;
    db.prepare(
        "INSERT OR IGNORE INTO playlist_tracks (playlist_id,track_id,position) VALUES (?,?,?)",
    )
    .bind_refs([
        &playlist,
        &track,
        &D1Type::Integer(i32::try_from(position).unwrap_or(i32::MAX)),
    ])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    Response::from_json(&load(&db, raw).await?.unwrap())
}

pub async fn remove_track(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let track_raw = match parse_id(&ctx, "track_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let playlist = match id_type(raw, "playlist id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let track = match id_type(track_raw, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let result = db
        .prepare("DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?")
        .bind_refs([&playlist, &track])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    if result
        .meta()
        .map_err(db_error)?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        == 0
    {
        return error_response(AppError::NotFound);
    }
    Response::from_json(&load(&db, raw).await?.unwrap())
}
