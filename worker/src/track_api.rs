mod metadata_update;

use serde::{Deserialize, Serialize};
use worker::{D1Type, Date, Request, Response, Result, RouteContext};

use crate::error::{bad_request, error_response, parse_id, parse_json};
use crate::multipart_api::AUDIO_OBJECT_KIND;
use crate::wasabi::{WasabiConfig, WasabiStorage};
use dantalian::application::error::AppError;
use dantalian::ports::object_storage::validate_object_key;

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

pub async fn put_book_metadata(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    metadata_update::put(req, ctx, "book_id").await
}

pub async fn put_cd_track_metadata(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    metadata_update::put(req, ctx, "cd_id").await
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

/// Request body for the presigned audio finalize step. The browser uploads the
/// object parts straight to Wasabi, then hands the completed multipart session
/// back so the track row and the derived audio jobs can be created.
#[derive(Debug, Deserialize)]
pub struct TrackAudioFinalizeRequest {
    pub session_id: String,
    /// Original client file name, recorded in `object_uploads` for later sync.
    #[serde(default)]
    pub name: Option<String>,
}

/// Object key and session details of a completed presigned multipart upload.
struct CompletedAudioUpload {
    object_key: String,
    file_hash: String,
    extension: String,
    object_kind: String,
    content_type: String,
    expected_size: i64,
    original_name: Option<String>,
}

/// Binds a completed presigned multipart audio object to a track.
///
/// The Worker never sees the audio bytes on this path: it verifies the session
/// metadata and the object size in Wasabi, records the object, updates the
/// track, and queues the data-saver jobs.
pub async fn finalize_cd_audio(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    finalize_track_audio(req, ctx, "cd_id").await
}

pub async fn finalize_book_audio(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    finalize_track_audio(req, ctx, "book_id").await
}

async fn finalize_track_audio(
    mut req: Request,
    ctx: RouteContext<()>,
    parent_column: &str,
) -> Result<Response> {
    let raw_parent = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let raw_track = match parse_id(&ctx, "track_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let parent = match id_type(raw_parent, "parent id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let track = match id_type(raw_track, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match parse_json::<TrackAudioFinalizeRequest>(&mut req).await {
        Ok(body) => body,
        Err(response) => return Ok(response),
    };
    if !valid_session_id(&body.session_id) {
        return Ok(bad_request("invalid multipart session id"));
    }
    let db = ctx.d1("DB")?;
    let sql = format!("SELECT id FROM tracks WHERE id = ? AND {parent_column} = ?");
    if db
        .prepare(&sql)
        .bind_refs([&track, &parent])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return error_response(AppError::NotFound);
    }
    let session = match load_completed_audio_session(&ctx, &body.session_id, body.name).await {
        Ok(session) => session,
        Err(error) => return error_response(error),
    };
    let config = match WasabiConfig::from_env(&ctx.env).await {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let storage = WasabiStorage::new(config);
    let stored = match storage.head_object(&session.object_key).await {
        Ok(metadata) => metadata.content_length,
        Err(error) => return error_response(error),
    };
    if stored != Some(u64::try_from(session.expected_size).unwrap_or(u64::MAX)) {
        return error_response(AppError::Conflict(
            "uploaded audio object size does not match the multipart session".to_string(),
        ));
    }
    let file_name = format!("{}.{}", session.file_hash, session.extension);
    let created_at = Date::now().as_millis().to_string();
    // Claim the session before touching the media rows: only the first finalize
    // can move complete -> finalized, so one object cannot be attached twice.
    let claimed = db
        .prepare("UPDATE multipart_uploads SET status = 'finalized' WHERE id = ? AND status = 'complete'")
        .bind_refs(&D1Type::Text(&body.session_id))
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    if claimed
        .meta()
        .map_err(db_error)?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        == 0
    {
        return error_response(AppError::Conflict(
            "multipart session has already been attached to a track".to_string(),
        ));
    }
    // `tracks` has no updated_at column; the media tables keep only the values.
    db.prepare("UPDATE tracks SET file_hash = ?, file_name = ? WHERE id = ?")
        .bind_refs([
            &D1Type::Text(&session.file_hash),
            &D1Type::Text(&file_name),
            &track,
        ])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    db.prepare(
        "INSERT OR REPLACE INTO object_uploads
         (object_key, object_kind, entity_id, content_type, extension, expected_size, original_name, status, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'complete', ?)",
    )
    .bind_refs([
        &D1Type::Text(&session.object_key),
        &D1Type::Text(AUDIO_OBJECT_KIND),
        &track,
        &D1Type::Text(&session.content_type),
        &D1Type::Text(&session.extension),
        &D1Type::Integer(i32::try_from(session.expected_size).unwrap_or(i32::MAX)),
        &session
            .original_name
            .as_deref()
            .map(D1Type::Text)
            .unwrap_or(D1Type::Null),
        &D1Type::Text(&created_at),
    ])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    // Data-saver encoding runs in the external processor against the object
    // that now exists in Wasabi, so the request path stays small.
    if let Err(error) = crate::audio_job_api::enqueue_data_saver_job_for_source(
        &ctx.env,
        &session.file_hash,
        &session.extension,
    )
    .await
    {
        worker::console_error!("data saver job scheduling deferred after finalize: {error}");
    }
    Response::from_json(&serde_json::json!({
        "file_hash": session.file_hash,
        "file_name": file_name,
        "object_key": session.object_key,
        "object_kind": session.object_kind,
        "size": session.expected_size,
    }))
}

/// Loads a multipart session that has already been completed against Wasabi and
/// proves it is an audio object of the canonical `audio/<hash>.<extension>`
/// shape, so the track row can be derived from the session alone.
async fn load_completed_audio_session(
    ctx: &RouteContext<()>,
    session_id: &str,
    original_name: Option<String>,
) -> std::result::Result<CompletedAudioUpload, AppError> {
    let db = ctx
        .d1("DB")
        .map_err(|error| AppError::Database(error.to_string()))?;
    let row = db
        .prepare(
            "SELECT object_key, object_kind, expected_size, content_type, status
             FROM multipart_uploads WHERE id = ?",
        )
        .bind_refs(&D1Type::Text(session_id))
        .map_err(|error| AppError::Database(error.to_string()))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|error| AppError::Database(error.to_string()))?;
    let Some(row) = row else {
        return Err(AppError::NotFound);
    };
    if row.get("status").and_then(|value| value.as_str()) != Some("complete") {
        return Err(AppError::Conflict(
            "multipart session is not complete".to_string(),
        ));
    }
    if row.get("object_kind").and_then(|value| value.as_str()) != Some(AUDIO_OBJECT_KIND) {
        return Err(AppError::Validation(
            "multipart session is not an audio upload".to_string(),
        ));
    }
    let object_key = row
        .get("object_key")
        .and_then(|value| value.as_str())
        .ok_or_else(|| AppError::Database("multipart session has no object key".to_string()))?
        .to_string();
    let content_type = row
        .get("content_type")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Database("multipart session has no content type".to_string()))?
        .to_string();
    let expected_size = row
        .get("expected_size")
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| AppError::Database("multipart session has no size".to_string()))?;
    let (file_hash, extension) = audio_key_parts(&object_key)?;
    Ok(CompletedAudioUpload {
        object_key,
        file_hash,
        extension,
        object_kind: AUDIO_OBJECT_KIND.to_string(),
        content_type,
        expected_size,
        original_name: original_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty() && name.len() <= 512),
    })
}

/// `audio/<hash>.<extension>` (optionally behind a deployment prefix) is the
/// only key shape a track can reference, so anything else is rejected before it
/// can be written into the media tables.
fn audio_key_parts(object_key: &str) -> std::result::Result<(String, String), AppError> {
    validate_object_key(object_key)?;
    let components = object_key.split('/').collect::<Vec<_>>();
    // `<prefix>/audio/<name>` with an optional deployment prefix; encoded
    // variants live under `audio/encoded/...` and are never track sources.
    if components.len() < 2
        || components[components.len() - 2] != "audio"
        || components.contains(&"encoded")
    {
        return Err(AppError::Validation(
            "audio objects must live under the audio/ prefix".to_string(),
        ));
    }
    let name = components[components.len() - 1];
    let (file_hash, extension) = name
        .rsplit_once('.')
        .ok_or_else(|| AppError::Validation("invalid audio object name".to_string()))?;
    if file_hash.is_empty()
        || !file_hash
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(AppError::Validation(
            "invalid audio object name".to_string(),
        ));
    }
    // Reuses the audio job extension contract so the finalize step and the
    // data-saver enqueue agree on the accepted extensions.
    let extension = crate::audio_job_api::normalize_extension(extension)
        .ok_or_else(|| AppError::Validation("invalid audio extension".to_string()))?;
    Ok((file_hash.to_string(), extension))
}

fn valid_session_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{audio_key_parts, custom_metadata_value, merge_custom_metadata, valid_session_id};
    use serde_json::json;

    #[test]
    fn audio_key_parts_accepts_only_canonical_audio_objects() {
        assert_eq!(
            audio_key_parts("audio/abc123.flac").expect("canonical key"),
            ("abc123".to_string(), "flac".to_string())
        );
        assert_eq!(
            audio_key_parts("dantalian/production/audio/abc123.MP3").expect("prefixed key"),
            ("abc123".to_string(), "mp3".to_string())
        );
        assert!(audio_key_parts("epubs/abc123.epub").is_err());
        assert!(audio_key_parts("audio/encoded/opus/abc123.opus").is_err());
        assert!(audio_key_parts("audio/abc123").is_err());
        assert!(audio_key_parts("audio/../secret.flac").is_err());
        assert!(audio_key_parts("audio/abc123.toolongextension").is_err());
        assert!(valid_session_id("0123456789abcdef0123456789abcdef"));
        assert!(!valid_session_id("0123"));
        assert!(!valid_session_id("0123456789abcdef0123456789abcdeZ"));
    }

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
