use serde::{Deserialize, Serialize};
use worker::{D1Type, Request, Response, Result, RouteContext};

use crate::error::{bad_request, error_response, parse_id, parse_json};
use dantalian::application::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
struct TrackRow {
    id: i64,
    book_id: Option<i64>,
    cd_id: Option<i64>,
    disc_number: i64,
    track_number: i64,
    title: String,
    duration: Option<String>,
    file_hash: Option<String>,
    file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrackRequest {
    disc_number: Option<i64>,
    track_number: Option<i64>,
    title: Option<String>,
    duration: Option<String>,
    swap_track_id: Option<i64>,
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

async fn list_for(ctx: &RouteContext<()>, column: &str, param: &str) -> Result<Response> {
    let raw = match parse_id(ctx, param) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, param) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let sql = format!(
        "SELECT id, book_id, cd_id, disc_number, track_number, title, duration, file_hash, file_name FROM tracks WHERE {column} = ? ORDER BY disc_number, track_number, id"
    );
    let rows = ctx
        .d1("DB")?
        .prepare(&sql)
        .bind_refs(&id)
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<TrackRow>()
        .map_err(db_error)?;
    Response::from_json(&rows)
}

pub async fn list_book(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    list_for(&ctx, "book_id", "id").await
}
pub async fn list_cd(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    list_for(&ctx, "cd_id", "id").await
}

pub async fn add_cd(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let cd_id = match id_type(raw, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match parse_json::<TrackRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let title = body.title.unwrap_or_default().trim().to_string();
    let track_number = body.track_number.unwrap_or_default();
    if title.is_empty() || track_number <= 0 {
        return Ok(bad_request("title and positive track_number are required"));
    }
    let disc_number = body.disc_number.unwrap_or(1).max(1);
    let duration = body.duration.unwrap_or_default();
    let row = ctx.d1("DB")?.prepare("INSERT INTO tracks (cd_id, disc_number, track_number, title, duration) VALUES (?, ?, ?, ?, ?) RETURNING id, book_id, cd_id, disc_number, track_number, title, duration, file_hash, file_name")
        .bind_refs([
            &cd_id,
            &D1Type::Integer(i32::try_from(disc_number).unwrap_or(i32::MAX)),
            &D1Type::Integer(i32::try_from(track_number).unwrap_or(i32::MAX)),
            &D1Type::Text(&title),
            &D1Type::Text(&duration),
        ]).map_err(db_error)?.first::<TrackRow>(None).await.map_err(db_error)?;
    match row {
        Some(row) => Response::from_json(&row).map(|response| response.with_status(201)),
        None => error_response(AppError::Internal("track insert returned no row".into())),
    }
}

async fn update_for(
    mut req: Request,
    ctx: RouteContext<()>,
    parent_column: &str,
) -> Result<Response> {
    let parent_raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let track_raw = match parse_id(&ctx, "track_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let parent_id = match id_type(parent_raw, "parent id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let track_id = match id_type(track_raw, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match parse_json::<TrackRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let current = db
        .prepare(&format!(
            "SELECT disc_number, track_number, title, duration
             FROM tracks WHERE id = ? AND {parent_column} = ?"
        ))
        .bind_refs([&track_id, &parent_id])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    let Some(current) = current else {
        return error_response(AppError::NotFound);
    };
    if let Some(swap_raw) = body.swap_track_id {
        let swap_id = match id_type(swap_raw, "swap track id") {
            Ok(value) => value,
            Err(error) => return error_response(error),
        };
        let swap = db
            .prepare(&format!(
                "SELECT track_number FROM tracks WHERE id = ? AND {parent_column} = ?"
            ))
            .bind_refs([&swap_id, &parent_id])
            .map_err(db_error)?
            .first::<serde_json::Value>(None)
            .await
            .map_err(db_error)?;
        let Some(swap) = swap else {
            return error_response(AppError::NotFound);
        };
        let first_number = current
            .get("track_number")
            .and_then(|value| value.as_i64())
            .unwrap_or(1);
        let second_number = swap
            .get("track_number")
            .and_then(|value| value.as_i64())
            .unwrap_or(1);
        db.prepare("UPDATE tracks SET track_number = ? WHERE id = ?")
            .bind_refs([
                &D1Type::Integer(i32::try_from(second_number).unwrap_or(i32::MAX)),
                &track_id,
            ])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
        db.prepare("UPDATE tracks SET track_number = ? WHERE id = ?")
            .bind_refs([
                &D1Type::Integer(i32::try_from(first_number).unwrap_or(i32::MAX)),
                &swap_id,
            ])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
        return Ok(Response::empty()?.with_status(204));
    }
    let disc_number = body
        .disc_number
        .or_else(|| current.get("disc_number").and_then(|value| value.as_i64()))
        .unwrap_or(1)
        .max(1);
    let track_number = body
        .track_number
        .or_else(|| current.get("track_number").and_then(|value| value.as_i64()))
        .unwrap_or(1)
        .max(1);
    let title = body
        .title
        .or_else(|| {
            current
                .get("title")
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
        })
        .unwrap_or_default();
    let duration = body
        .duration
        .or_else(|| {
            current
                .get("duration")
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
        })
        .unwrap_or_default();
    db.prepare(
        "UPDATE tracks SET disc_number = ?, track_number = ?, title = ?, duration = ? WHERE id = ?",
    )
    .bind_refs([
        &D1Type::Integer(i32::try_from(disc_number).unwrap_or(i32::MAX)),
        &D1Type::Integer(i32::try_from(track_number).unwrap_or(i32::MAX)),
        &D1Type::Text(&title),
        &D1Type::Text(&duration),
        &track_id,
    ])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}

pub async fn update_book(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    update_for(req, ctx, "book_id").await
}

pub async fn update_cd(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    update_for(req, ctx, "cd_id").await
}

async fn delete_for(ctx: &RouteContext<()>, parent_column: &str) -> Result<Response> {
    let parent_raw = match parse_id(ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let track_raw = match parse_id(ctx, "track_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let parent_id = match id_type(parent_raw, "parent id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let track_id = match id_type(track_raw, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let result = ctx
        .d1("DB")?
        .prepare(&format!(
            "DELETE FROM tracks WHERE id = ? AND {parent_column} = ?"
        ))
        .bind_refs([&track_id, &parent_id])
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

pub async fn delete_book(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    delete_for(&ctx, "book_id").await
}

pub async fn delete_cd(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    delete_for(&ctx, "cd_id").await
}

fn text_value<'a>(body: &'a serde_json::Value, key: &str) -> D1Type<'a> {
    body.get(key)
        .and_then(|value| value.as_str())
        .map(D1Type::Text)
        .unwrap_or(D1Type::Null)
}
fn int_value(body: &serde_json::Value, key: &str) -> D1Type<'static> {
    body.get(key)
        .and_then(|value| value.as_i64())
        .and_then(|value| i32::try_from(value).ok())
        .map(D1Type::Integer)
        .unwrap_or(D1Type::Null)
}
fn real_value(body: &serde_json::Value, key: &str) -> D1Type<'static> {
    body.get(key)
        .and_then(|value| value.as_f64())
        .map(D1Type::Real)
        .unwrap_or(D1Type::Null)
}
const MAX_CUSTOM_METADATA_BYTES: usize = 256 * 1024;

fn custom_metadata_value(body: &serde_json::Value) -> Result<String, AppError> {
    let mut custom = serde_json::Map::new();
    for key in [
        "duration_seconds",
        "sample_rate",
        "channels",
        "bitrate_kbps",
        "tags",
    ] {
        if let Some(value) = body.get(key) {
            custom.insert(key.to_string(), value.clone());
        }
    }
    let serialized = serde_json::to_string(&serde_json::Value::Object(custom))
        .map_err(|error| AppError::Validation(format!("invalid custom metadata: {error}")))?;
    if serialized.len() > MAX_CUSTOM_METADATA_BYTES {
        return Err(AppError::Validation(
            "custom audio metadata is too large".to_string(),
        ));
    }
    Ok(serialized)
}
fn merge_custom_metadata(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let custom_json = object
        .remove("custom_json")
        .and_then(|value| value.as_str().map(str::to_owned));
    let Some(custom_json) = custom_json else {
        return;
    };
    let Ok(serde_json::Value::Object(custom)) =
        serde_json::from_str::<serde_json::Value>(&custom_json)
    else {
        return;
    };
    for (key, value) in custom {
        object.entry(key).or_insert(value);
    }
}

async fn get_metadata_for(ctx: &RouteContext<()>, parent_column: &str) -> Result<Response> {
    let parent_raw = match parse_id(ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let track_raw = match parse_id(ctx, "track_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let parent_id = match id_type(parent_raw, "parent id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let track_id = match id_type(track_raw, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let track = db
        .prepare(&format!(
            "SELECT id, cd_id FROM tracks WHERE id = ? AND {parent_column} = ?"
        ))
        .bind_refs([&track_id, &parent_id])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    let Some(track) = track else {
        return error_response(AppError::NotFound);
    };
    let mut value = db
        .prepare("SELECT * FROM track_metadata WHERE track_id = ?")
        .bind_refs(&track_id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .unwrap_or_else(|| serde_json::json!({ "track_id": track_raw }));
    merge_custom_metadata(&mut value);
    if let Some(cd_id) = track.get("cd_id").and_then(|value| value.as_i64()) {
        let cd_id =
            id_type(cd_id, "cd id").map_err(|error| worker::Error::from(error.to_string()))?;
        let cd = db
            .prepare("SELECT title, publisher, label FROM cds WHERE id = ?")
            .bind_refs(&cd_id)
            .map_err(db_error)?
            .first::<serde_json::Value>(None)
            .await
            .map_err(db_error)?;
        let cd_meta = db
            .prepare("SELECT year, genre, composer, isrc FROM cd_metadata WHERE cd_id = ?")
            .bind_refs(&cd_id)
            .map_err(db_error)?
            .first::<serde_json::Value>(None)
            .await
            .map_err(db_error)?;
        if let Some(object) = value.as_object_mut() {
            for (target, source) in [
                ("album", "title"),
                ("publisher", "publisher"),
                ("label", "label"),
            ] {
                if object.get(target).is_none_or(serde_json::Value::is_null) {
                    if let Some(source_value) = cd.as_ref().and_then(|row| row.get(source)) {
                        object.insert(target.to_string(), source_value.clone());
                    }
                }
            }
            for key in ["year", "genre", "composer", "isrc"] {
                if object.get(key).is_none_or(serde_json::Value::is_null) {
                    if let Some(source_value) = cd_meta.as_ref().and_then(|row| row.get(key)) {
                        object.insert(key.to_string(), source_value.clone());
                    }
                }
            }
        }
    }
    let artists = db
        .prepare(
            "SELECT a.id, a.ndl_id, a.name, a.transcription, ta.sort_order
             FROM authors a JOIN track_authors ta ON ta.author_id = a.id
             WHERE ta.track_id = ? ORDER BY ta.sort_order, ta.author_id",
        )
        .bind_refs(&track_id)
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<serde_json::Value>()
        .map_err(db_error)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("artists".into(), serde_json::Value::Array(artists));
        object
            .entry("album_artists")
            .or_insert_with(|| serde_json::json!([]));
    }
    Response::from_json(&value)
}

pub async fn get_book_metadata(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    get_metadata_for(&ctx, "book_id").await
}

pub async fn get_cd_track_metadata(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    get_metadata_for(&ctx, "cd_id").await
}

async fn put_metadata_for(
    mut req: Request,
    ctx: RouteContext<()>,
    parent_column: &str,
) -> Result<Response> {
    let parent_raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let track_raw = match parse_id(&ctx, "track_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let parent_id = match id_type(parent_raw, "parent id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let track_id = match id_type(track_raw, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match req.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(error) => return Ok(bad_request(format!("invalid JSON: {error}"))),
    };
    let db = ctx.d1("DB")?;
    let exists = db
        .prepare(&format!(
            "SELECT id FROM tracks WHERE id = ? AND {parent_column} = ?"
        ))
        .bind_refs([&track_id, &parent_id])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .is_some();
    if !exists {
        return error_response(AppError::NotFound);
    }
    let custom_json = match custom_metadata_value(&body) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let values = [
        track_id,
        text_value(&body, "title"),
        text_value(&body, "artist"),
        text_value(&body, "album"),
        text_value(&body, "album_artist"),
        int_value(&body, "track_number"),
        int_value(&body, "track_total"),
        int_value(&body, "disc_number"),
        int_value(&body, "disc_total"),
        int_value(&body, "year"),
        text_value(&body, "genre"),
        text_value(&body, "composer"),
        text_value(&body, "publisher"),
        text_value(&body, "label"),
        text_value(&body, "encoder"),
        text_value(&body, "comment"),
        text_value(&body, "lyrics"),
        real_value(&body, "replay_gain_track_gain_db"),
        real_value(&body, "replay_gain_track_peak"),
        real_value(&body, "replay_gain_album_gain_db"),
        real_value(&body, "replay_gain_album_peak"),
        text_value(&body, "file_type"),
        int_value(&body, "raw_size_bytes"),
        D1Type::Text(&custom_json),
    ];
    db.prepare(
        "INSERT INTO track_metadata
         (track_id,title,artist,album,album_artist,track_number,track_total,disc_number,disc_total,
          year,genre,composer,publisher,label,encoder,comment,lyrics,replay_gain_track_gain_db,
          replay_gain_track_peak,replay_gain_album_gain_db,replay_gain_album_peak,file_type,raw_size_bytes,custom_json)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
         ON CONFLICT(track_id) DO UPDATE SET
          title=COALESCE(excluded.title,track_metadata.title),
          artist=COALESCE(excluded.artist,track_metadata.artist),
          album=COALESCE(excluded.album,track_metadata.album),
          album_artist=COALESCE(excluded.album_artist,track_metadata.album_artist),
          track_number=COALESCE(excluded.track_number,track_metadata.track_number),
          track_total=COALESCE(excluded.track_total,track_metadata.track_total),
          disc_number=COALESCE(excluded.disc_number,track_metadata.disc_number),
          disc_total=COALESCE(excluded.disc_total,track_metadata.disc_total),
          year=COALESCE(excluded.year,track_metadata.year),
          genre=COALESCE(excluded.genre,track_metadata.genre),
          composer=COALESCE(excluded.composer,track_metadata.composer),
          publisher=COALESCE(excluded.publisher,track_metadata.publisher),
          label=COALESCE(excluded.label,track_metadata.label),
          encoder=COALESCE(excluded.encoder,track_metadata.encoder),
          comment=COALESCE(excluded.comment,track_metadata.comment),
          lyrics=COALESCE(excluded.lyrics,track_metadata.lyrics),
          replay_gain_track_gain_db=COALESCE(excluded.replay_gain_track_gain_db,track_metadata.replay_gain_track_gain_db),
          replay_gain_track_peak=COALESCE(excluded.replay_gain_track_peak,track_metadata.replay_gain_track_peak),
          replay_gain_album_gain_db=COALESCE(excluded.replay_gain_album_gain_db,track_metadata.replay_gain_album_gain_db),
          replay_gain_album_peak=COALESCE(excluded.replay_gain_album_peak,track_metadata.replay_gain_album_peak),
          file_type=COALESCE(excluded.file_type,track_metadata.file_type),
          raw_size_bytes=COALESCE(excluded.raw_size_bytes,track_metadata.raw_size_bytes),
          custom_json=CASE WHEN excluded.custom_json <> '{}' THEN excluded.custom_json ELSE track_metadata.custom_json END,
          updated_at=CURRENT_TIMESTAMP",
    )
    .bind_refs(values.iter())
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    let track_id =
        id_type(track_raw, "track id").map_err(|error| worker::Error::from(error.to_string()))?;
    if let Some(artists) = body.get("artists").and_then(|value| value.as_array()) {
        db.prepare("DELETE FROM track_authors WHERE track_id = ?")
            .bind_refs(&track_id)
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
        for (sort_order, author_id) in artists
            .iter()
            .filter_map(|value| value.as_i64())
            .enumerate()
        {
            let author_id = match id_type(author_id, "author id") {
                Ok(value) => value,
                Err(error) => return error_response(error),
            };
            db.prepare(
                "INSERT OR IGNORE INTO track_authors (track_id, author_id, sort_order)
                 VALUES (?, ?, ?)",
            )
            .bind_refs([
                &track_id,
                &author_id,
                &D1Type::Integer(i32::try_from(sort_order).unwrap_or(i32::MAX)),
            ])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
        }
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn put_book_metadata(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    put_metadata_for(req, ctx, "book_id").await
}

pub async fn put_cd_track_metadata(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    put_metadata_for(req, ctx, "cd_id").await
}

pub async fn get_cd_metadata(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let cd_id = match id_type(raw, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let row = ctx
        .d1("DB")?
        .prepare("SELECT * FROM cd_metadata WHERE cd_id = ?")
        .bind_refs(&cd_id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    Response::from_json(&row.unwrap_or_else(|| serde_json::json!({"cd_id": raw})))
}
pub async fn put_cd_metadata(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let cd_id = match id_type(raw, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match req.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(error) => return Ok(bad_request(format!("invalid JSON: {error}"))),
    };
    let db = ctx.d1("DB")?;
    if db
        .prepare("SELECT id FROM cds WHERE id = ?")
        .bind_refs(&cd_id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return error_response(AppError::NotFound);
    }
    let mut assignments = Vec::new();
    let mut values = Vec::with_capacity(5);
    values.push(cd_id);
    for key in ["year", "genre", "composer", "isrc"] {
        let value = body.get(key);
        if value.is_some() {
            assignments.push(format!("{key} = excluded.{key}"));
        }
        let parsed = match key {
            "year" => value
                .and_then(|value| {
                    if value.is_null() {
                        Some(D1Type::Null)
                    } else {
                        value
                            .as_i64()
                            .and_then(|value| i32::try_from(value).ok())
                            .map(D1Type::Integer)
                    }
                })
                .unwrap_or(D1Type::Null),
            _ => value
                .and_then(|value| {
                    if value.is_null() {
                        Some(D1Type::Null)
                    } else {
                        value.as_str().map(D1Type::Text)
                    }
                })
                .unwrap_or(D1Type::Null),
        };
        values.push(parsed);
    }
    if assignments.is_empty() {
        return Ok(Response::empty()?.with_status(204));
    }
    let sql = format!(
        "INSERT INTO cd_metadata (cd_id,year,genre,composer,isrc)
         VALUES (?,?,?,?,?)
         ON CONFLICT(cd_id) DO UPDATE SET {}, updated_at=CURRENT_TIMESTAMP",
        assignments.join(", ")
    );
    db.prepare(&sql)
        .bind_refs(values.iter())
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}

pub async fn album_tags(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let cd_id = match id_type(raw, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let row = ctx.d1("DB")?.prepare("SELECT (SELECT album FROM track_metadata tm JOIN tracks t ON t.id = tm.track_id WHERE t.cd_id = ? AND album IS NOT NULL AND album <> '' ORDER BY t.disc_number,t.track_number LIMIT 1) AS album, (SELECT album_artist FROM track_metadata tm JOIN tracks t ON t.id = tm.track_id WHERE t.cd_id = ? AND album_artist IS NOT NULL AND album_artist <> '' ORDER BY t.disc_number,t.track_number LIMIT 1) AS album_artist, (SELECT artist FROM track_metadata tm JOIN tracks t ON t.id = tm.track_id WHERE t.cd_id = ? AND artist IS NOT NULL AND artist <> '' ORDER BY t.disc_number,t.track_number LIMIT 1) AS artist").bind_refs([&cd_id, &cd_id, &cd_id]).map_err(db_error)?.first::<serde_json::Value>(None).await.map_err(db_error)?;
    Response::from_json(&row.unwrap_or_else(|| serde_json::json!({})))
}

pub async fn search(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::from_json(&Vec::<serde_json::Value>::new())
}

#[cfg(test)]
mod tests {
    use super::{custom_metadata_value, merge_custom_metadata};
    use serde_json::json;

    #[test]
    fn custom_metadata_round_trips_technical_fields() {
        let body = json!({
            "title": "Track",
            "duration_seconds": 856.96,
            "sample_rate": 44_100,
            "channels": 2,
            "bitrate_kbps": 699,
            "tags": {"tracknumber": "02"}
        });
        let custom = custom_metadata_value(&body).expect("custom metadata");
        let mut stored = json!({"custom_json": custom});
        merge_custom_metadata(&mut stored);
        assert_eq!(stored["duration_seconds"], 856.96);
        assert!(stored.get("custom_json").is_none());
        assert_eq!(stored["sample_rate"], 44_100);
        assert_eq!(stored["channels"], 2);
        assert_eq!(stored["bitrate_kbps"], 699);
        assert_eq!(stored["tags"]["tracknumber"], "02");
    }

    #[test]
    fn custom_metadata_rejects_oversized_tags() {
        let body = json!({"tags": {"large": "x".repeat(256 * 1024)}});
        assert!(custom_metadata_value(&body).is_err());
    }
}
