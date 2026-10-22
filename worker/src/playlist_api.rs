mod storage;

use crate::d1_read;
use crate::error::{bad_request, error_response, parse_id, parse_json};
use dantalian::application::error::AppError;
use dantalian::domain::patch::deserialize_nullable;
use serde::Deserialize;
use std::collections::HashSet;
use worker::{Request, Response, Result, RouteContext};

#[derive(Deserialize)]
struct CreateRequest {
    name: String,
    description: Option<String>,
    cover_cd_id: Option<i64>,
    #[serde(default)]
    track_ids: Vec<i64>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    cover_cd_id: Option<Option<i64>>,
    track_ids: Option<Vec<i64>>,
}

#[derive(Deserialize)]
struct TrackRequest {
    track_id: Option<i64>,
    track_ids: Option<Vec<i64>>,
}

#[derive(Deserialize)]
struct SetTracksRequest {
    track_ids: Vec<i64>,
}

fn valid_id(id: i64) -> bool {
    id > 0 && i32::try_from(id).is_ok()
}

fn path_id(ctx: &RouteContext<()>, key: &str) -> std::result::Result<i64, Response> {
    let id = parse_id(ctx, key)?;
    if !valid_id(id) {
        return Err(bad_request(format!("invalid {key}")));
    }
    Ok(id)
}

fn normalize_ids(ids: &mut Vec<i64>) -> std::result::Result<(), Response> {
    if ids.iter().any(|&id| !valid_id(id)) {
        return Err(bad_request("invalid track id"));
    }
    let mut seen = HashSet::with_capacity(ids.len());
    ids.retain(|id| seen.insert(*id));
    Ok(())
}

fn name(value: &str) -> std::result::Result<&str, Response> {
    let value = value.trim();
    if value.is_empty() {
        return Err(bad_request("playlist name is required"));
    }
    if value.chars().count() > 200 {
        return Err(bad_request("playlist name is too long"));
    }
    Ok(value)
}

async fn validate(
    db: &worker::D1Database,
    cover: Option<i64>,
    tracks: Option<&[i64]>,
) -> Result<Option<Response>> {
    if let Some(cover) = cover {
        if !valid_id(cover) || !storage::valid_cover(db, cover).await? {
            return Ok(Some(bad_request("cover CD not found")));
        }
    }
    if let Some(tracks) = tracks {
        if !storage::valid_tracks(db, tracks).await? {
            return Ok(Some(bad_request(
                "playlist contains an invalid audio track",
            )));
        }
    }
    Ok(None)
}

async fn output(db: &worker::D1Database, id: i64) -> Result<Response> {
    match storage::load(db, id).await? {
        Some(value) => Response::from_json(&value),
        None => error_response(AppError::NotFound),
    }
}

async fn write_error(
    db: &worker::D1Database,
    id: Option<i64>,
    error: worker::Error,
) -> Result<Response> {
    if let Some(id) = id {
        if storage::load(db, id).await?.is_none() {
            return error_response(AppError::NotFound);
        }
    }
    if error
        .to_string()
        .contains("NOT NULL constraint failed: playlist_tracks.track_id")
    {
        return Ok(bad_request("playlist contains an invalid audio track"));
    }
    Err(error)
}

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DB")?;
    let db = d1_read::read_db(&req, &db)?;
    Response::from_json(&storage::list(&db).await?)
}

pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match path_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    output(&ctx.d1("DB")?, id).await
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let mut body = match parse_json::<CreateRequest>(&mut req).await {
        Ok(body) => body,
        Err(response) => return Ok(response),
    };
    let name = match name(&body.name) {
        Ok(name) => name,
        Err(response) => return Ok(response),
    };
    if let Err(response) = normalize_ids(&mut body.track_ids) {
        return Ok(response);
    }
    let db = ctx.d1("DB")?;
    if let Some(response) = validate(&db, body.cover_cd_id, Some(&body.track_ids)).await? {
        return Ok(response);
    }
    let id = match storage::create(
        &db,
        name,
        body.description.as_deref(),
        body.cover_cd_id,
        &body.track_ids,
    )
    .await
    {
        Ok(id) => id,
        Err(error) => return write_error(&db, None, error).await,
    };
    output(&db, id).await.map(|response| {
        if response.status_code() == 200 {
            response.with_status(201)
        } else {
            response
        }
    })
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match path_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let mut body = match parse_json::<UpdateRequest>(&mut req).await {
        Ok(body) => body,
        Err(response) => return Ok(response),
    };
    let name = match body.name.as_deref().map(name).transpose() {
        Ok(name) => name,
        Err(response) => return Ok(response),
    };
    if let Some(ids) = &mut body.track_ids {
        if let Err(response) = normalize_ids(ids) {
            return Ok(response);
        }
    }
    let db = ctx.d1("DB")?;
    if storage::load(&db, id).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    if let Some(response) =
        validate(&db, body.cover_cd_id.flatten(), body.track_ids.as_deref()).await?
    {
        return Ok(response);
    }
    if let Err(error) = storage::update(
        &db,
        id,
        name,
        body.description.as_ref().map(|value| value.as_deref()),
        body.cover_cd_id,
        body.track_ids.as_deref(),
    )
    .await
    {
        return write_error(&db, Some(id), error).await;
    }
    output(&db, id).await
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match path_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    if storage::delete(&ctx.d1("DB")?, id).await? {
        Ok(Response::empty()?.with_status(204))
    } else {
        error_response(AppError::NotFound)
    }
}

pub async fn set_tracks(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match path_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let mut body = match parse_json::<SetTracksRequest>(&mut req).await {
        Ok(body) => body,
        Err(response) => return Ok(response),
    };
    if let Err(response) = normalize_ids(&mut body.track_ids) {
        return Ok(response);
    }
    let db = ctx.d1("DB")?;
    if storage::load(&db, id).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    if let Some(response) = validate(&db, None, Some(&body.track_ids)).await? {
        return Ok(response);
    }
    if let Err(error) = storage::update(&db, id, None, None, None, Some(&body.track_ids)).await {
        return write_error(&db, Some(id), error).await;
    }
    output(&db, id).await
}

pub async fn add_track(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match path_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let body = match parse_json::<TrackRequest>(&mut req).await {
        Ok(body) => body,
        Err(response) => return Ok(response),
    };
    let mut tracks = match (body.track_id, body.track_ids) {
        (Some(id), None) => vec![id],
        (None, Some(ids)) => ids,
        _ => return Ok(bad_request("specify track_id or track_ids")),
    };
    if let Err(response) = normalize_ids(&mut tracks) {
        return Ok(response);
    }
    let db = ctx.d1("DB")?;
    if storage::load(&db, id).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    if let Some(response) = validate(&db, None, Some(&tracks)).await? {
        return Ok(response);
    }
    if let Err(error) = storage::append(&db, id, &tracks).await {
        return write_error(&db, Some(id), error).await;
    }
    output(&db, id).await
}

pub async fn remove_tracks(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match path_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let mut body = match parse_json::<SetTracksRequest>(&mut req).await {
        Ok(body) => body,
        Err(response) => return Ok(response),
    };
    if let Err(response) = normalize_ids(&mut body.track_ids) {
        return Ok(response);
    }
    let db = ctx.d1("DB")?;
    if storage::load(&db, id).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    storage::remove(&db, id, &body.track_ids).await?;
    output(&db, id).await
}

pub async fn remove_track(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match path_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let track = match path_id(&ctx, "track_id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    if storage::load(&db, id).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    if storage::remove(&db, id, &[track]).await? == 0 {
        return error_response(AppError::NotFound);
    }
    output(&db, id).await
}
