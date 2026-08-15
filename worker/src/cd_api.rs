use serde::{Deserialize, Serialize};
use worker::{D1Type, Request, Response, Result, RouteContext};

use crate::amazon_api;
use crate::error::{bad_request, error_response, parse_id, parse_json};
use crate::musicbrainz_api;
use dantalian::application::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
struct CdRow {
    id: i64,
    jan: Option<String>,
    title: String,
    artist: Option<String>,
    publisher: Option<String>,
    label: Option<String>,
    catalog_number: Option<String>,
    publish_date: Option<String>,
    cover_url: Option<String>,
    description: Option<String>,
    disc_count: Option<i64>,
    volume: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    parent_book_id: Option<i64>,
    media_type: Option<String>,
    series_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CdRequest {
    jan: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    publisher: Option<String>,
    label: Option<String>,
    catalog_number: Option<String>,
    publish_date: Option<String>,
    cover_url: Option<String>,
    description: Option<String>,
    disc_count: Option<i64>,
    volume: Option<String>,
    parent_book_id: Option<i64>,
    media_type: Option<String>,
    series_id: Option<i64>,
    grand_series_id: Option<i64>,
    tracks: Option<Vec<TrackRequest>>,
    author_ids: Option<Vec<i64>>,
    metadata: Option<CdMetadataRequest>,
    musicbrainz_release_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrackRequest {
    disc_number: Option<i64>,
    track_number: i64,
    title: String,
    duration: Option<String>,
    #[serde(flatten)]
    metadata: serde_json::Map<String, serde_json::Value>,
}

fn metadata_text<'a>(
    metadata: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> D1Type<'a> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(D1Type::Text)
        .unwrap_or(D1Type::Null)
}

fn metadata_integer<'a>(
    metadata: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> D1Type<'a> {
    metadata
        .get(key)
        .and_then(|value| value.as_i64())
        .and_then(|value| i32::try_from(value).ok())
        .map(D1Type::Integer)
        .unwrap_or(D1Type::Null)
}

async fn insert_track_metadata(
    db: &worker::D1Database,
    track_id: i64,
    track: &TrackRequest,
) -> Result<()> {
    let track_id =
        id_type(track_id, "track id").map_err(|error| worker::Error::from(error.to_string()))?;
    let disc_number = track.disc_number.unwrap_or(1).max(1);
    let track_number = track.track_number;
    let title = D1Type::Text(&track.title);
    let artist = metadata_text(&track.metadata, "artist");
    let album = metadata_text(&track.metadata, "album");
    let album_artist = metadata_text(&track.metadata, "album_artist");
    let track_total = metadata_integer(&track.metadata, "track_total");
    let disc_total = metadata_integer(&track.metadata, "disc_total");
    let year = metadata_integer(&track.metadata, "year");
    let genre = metadata_text(&track.metadata, "genre");
    let composer = metadata_text(&track.metadata, "composer");
    let publisher = metadata_text(&track.metadata, "publisher");
    let label = metadata_text(&track.metadata, "label");
    let encoder = metadata_text(&track.metadata, "encoder");
    let comment = metadata_text(&track.metadata, "comment");
    let lyrics = metadata_text(&track.metadata, "lyrics");
    let file_type = metadata_text(&track.metadata, "file_type");
    let raw_size_bytes = metadata_integer(&track.metadata, "raw_size_bytes");
    db.prepare(
        "INSERT INTO track_metadata
         (track_id,title,artist,album,album_artist,track_number,track_total,
          disc_number,disc_total,year,genre,composer,publisher,label,encoder,
          comment,lyrics,file_type,raw_size_bytes)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind_refs([
        &track_id,
        &title,
        &artist,
        &album,
        &album_artist,
        &D1Type::Integer(i32::try_from(track_number).unwrap_or(i32::MAX)),
        &track_total,
        &D1Type::Integer(i32::try_from(disc_number).unwrap_or(i32::MAX)),
        &disc_total,
        &year,
        &genre,
        &composer,
        &publisher,
        &label,
        &encoder,
        &comment,
        &lyrics,
        &file_type,
        &raw_size_bytes,
    ])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CdMetadataRequest {
    year: Option<i64>,
    genre: Option<String>,
    composer: Option<String>,
    isrc: Option<String>,
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

async fn row(db: &worker::D1Database, id: &D1Type<'_>) -> Result<Option<CdRow>> {
    db.prepare("SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE id = ?")
        .bind_refs(id).map_err(db_error)?.first::<CdRow>(None).await.map_err(db_error)
}

async fn with_children(db: &worker::D1Database, cd: CdRow) -> Result<serde_json::Value> {
    let id = id_type(cd.id, "cd id").map_err(|error| worker::Error::from(error.to_string()))?;
    let tracks = db
        .prepare("SELECT id, book_id, cd_id, disc_number, track_number, title, duration, file_hash, file_name FROM tracks WHERE cd_id = ? ORDER BY disc_number, track_number, id")
        .bind_refs(&id)
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<serde_json::Value>()
        .map_err(db_error)?;
    let authors = db
        .prepare("SELECT a.id, a.ndl_id, a.name, a.transcription, ca.sort_order FROM authors a JOIN cd_authors ca ON ca.author_id = a.id WHERE ca.cd_id = ? ORDER BY ca.sort_order, ca.author_id")
        .bind_refs(&id)
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<serde_json::Value>()
        .map_err(db_error)?;
    let tags = db
        .prepare(
            "SELECT
                (SELECT artist FROM track_metadata tm
                 JOIN tracks t ON t.id = tm.track_id
                 WHERE t.cd_id = ? AND artist IS NOT NULL AND artist <> ''
                 ORDER BY t.disc_number, t.track_number LIMIT 1) AS track_artist,
                (SELECT album_artist FROM track_metadata tm
                 JOIN tracks t ON t.id = tm.track_id
                 WHERE t.cd_id = ? AND album_artist IS NOT NULL AND album_artist <> ''
                 ORDER BY t.disc_number, t.track_number LIMIT 1) AS album_artist",
        )
        .bind_refs([&id, &id])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .unwrap_or_default();
    let mut value =
        serde_json::to_value(cd).map_err(|error| worker::Error::from(error.to_string()))?;
    if let Some(object) = value.as_object_mut() {
        object.insert("tracks".into(), serde_json::Value::Array(tracks));
        object.insert("authors".into(), serde_json::Value::Array(authors));
        object.insert(
            "track_artist".into(),
            tags.get("track_artist")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        object.insert(
            "album_artist".into(),
            tags.get("album_artist")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
    }
    Ok(value)
}

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DB")?;
    let cds = db.prepare("SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, created_at, updated_at, parent_book_id, media_type, series_id FROM cds ORDER BY id DESC").all().await.map_err(db_error)?.results::<CdRow>().map_err(db_error)?;
    let mut result = Vec::with_capacity(cds.len());
    for cd in cds {
        result.push(with_children(&db, cd).await?);
    }
    Response::from_json(&result)
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let mut body = match parse_json::<CdRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let jan = body
        .jan
        .as_deref()
        .map(|value| value.replace(['-', ' ', '　'], ""))
        .filter(|value| !value.is_empty());
    let db = ctx.d1("DB")?;

    if let Some(jan) = jan.as_deref() {
        if let Some(existing) = db
            .prepare("SELECT id,jan,title,artist,publisher,label,catalog_number,publish_date,cover_url,description,disc_count,volume,created_at,updated_at,parent_book_id,media_type,series_id FROM cds WHERE jan = ?")
            .bind_refs(&D1Type::Text(jan))
            .map_err(db_error)?
            .first::<CdRow>(None)
            .await
            .map_err(db_error)?
        {
            let value = with_children(&db, existing).await?;
            return Response::from_json(&serde_json::json!({"cd": value}));
        }
    }

    let mut title = body.title.clone().filter(|value| !value.trim().is_empty());
    let mut artist = body.artist.clone();
    let mut publisher = body.publisher.clone();
    let mut label = body.label.clone();
    let mut catalog_number = body.catalog_number.clone();
    let mut publish_date = body.publish_date.clone();
    let mut cover_url = body.cover_url.clone();
    let mut disc_count = body.disc_count;
    let mut tracks = body.tracks.take();
    let mut source = "manual";

    let external_lookup = title.is_none();
    if title.is_none() {
        let Some(jan) = jan.as_deref() else {
            return Ok(bad_request("title or jan is required"));
        };
        let cd = if let Some(release_id) = body.musicbrainz_release_id.as_deref() {
            match musicbrainz_api::lookup_cd_by_release_id(release_id).await {
                Ok(Some(cd)) => cd,
                Ok(None) => {
                    return Response::from_json(&serde_json::json!({
                        "code": "cd_not_found",
                        "error": "MusicBrainz release not found"
                    }))
                    .map(|response| response.with_status(404));
                }
                Err(error) => {
                    return error_response(AppError::Internal(format!(
                        "MusicBrainz release lookup failed: {error}"
                    )));
                }
            }
        } else {
            let mut candidates = match musicbrainz_api::lookup_cd_candidates(jan).await {
                Ok(candidates) => candidates,
                Err(error) => {
                    worker::console_error!(
                        "MusicBrainz search failed for JAN; trying Amazon title fallback: {error}"
                    );
                    Vec::new()
                }
            };
            let mut amazon_title = None;
            if candidates.is_empty() {
                amazon_title = amazon_api::lookup_amazon_title_for_jan(jan)
                    .await
                    .ok()
                    .flatten();
                if let Some(title) = amazon_title.as_deref() {
                    candidates = musicbrainz_api::lookup_cd_candidates_by_title(title)
                        .await
                        .unwrap_or_default();
                }
            }
            match candidates.len() {
                0 => {
                    return Response::from_json(&serde_json::json!({
                        "code": "cd_not_found",
                        "error": "CD not found for JAN"
                    }))
                    .map(|response| response.with_status(404));
                }
                1 => {
                    let candidate = candidates.pop().expect("candidate length checked");
                    match musicbrainz_api::lookup_cd_candidate(&candidate.id).await {
                        Ok(Some(cd)) => cd,
                        Ok(None) => {
                            return Response::from_json(&serde_json::json!({
                                "code": "cd_not_found",
                                "error": "MusicBrainz release not found"
                            }))
                            .map(|response| response.with_status(404));
                        }
                        Err(error) => {
                            return error_response(AppError::Internal(format!(
                                "MusicBrainz release lookup failed: {error}"
                            )));
                        }
                    }
                }
                _ => {
                    let mut response = serde_json::json!({
                        "code": "musicbrainz_candidates",
                        "error": "MusicBrainzの候補を選択してください",
                        "jan": jan,
                        "candidates": candidates
                    });
                    if let Some(amazon_title) = amazon_title {
                        response["amazon_title"] = serde_json::Value::String(amazon_title);
                    }
                    return Response::from_json(&response)
                        .map(|response| response.with_status(300));
                }
            }
        };
        title = Some(cd.title);
        artist = cd.artist;
        publisher = cd.publisher;
        label = cd.label;
        catalog_number = cd.catalog_number;
        publish_date = cd.publish_date;
        cover_url = cd.cover_url;
        disc_count = cd.disc_count;
        tracks = Some(
            cd.tracks
                .into_iter()
                .map(|track| TrackRequest {
                    disc_number: Some(track.disc_number),
                    track_number: track.track_number,
                    title: track.title,
                    duration: track.duration,
                    metadata: serde_json::Map::new(),
                })
                .collect(),
        );
        source = "musicbrainz";
    }

    if external_lookup {
        match amazon_api::lookup_amazon_cover_for_jan(jan.as_deref().unwrap_or_default()).await {
            Ok(Some(cover)) => match amazon_api::persist_cover(&ctx, &cover).await {
                Ok(file_name) => {
                    cover_url = Some(file_name);
                    source = "amazon";
                }
                Err(error) => {
                    worker::console_error!(
                        "Amazon CD cover storage failed; keeping MusicBrainz metadata: {error}"
                    );
                }
            },
            Ok(None) => {}
            Err(error) => {
                worker::console_error!(
                    "Amazon CD cover lookup failed; keeping MusicBrainz metadata: {error}"
                );
            }
        }
    }

    let Some(title) = title else {
        return Ok(bad_request("title or jan is required"));
    };
    let jan_value = jan.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null);
    let row = db
        .prepare("INSERT INTO cds (jan,title,artist,publisher,label,catalog_number,publish_date,cover_url,description,disc_count,volume,parent_book_id,media_type,series_id) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?) RETURNING id,jan,title,artist,publisher,label,catalog_number,publish_date,cover_url,description,disc_count,volume,created_at,updated_at,parent_book_id,media_type,series_id")
        .bind_refs([
            &jan_value,
            &D1Type::Text(&title),
            &artist.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &publisher.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &label.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &catalog_number.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &publish_date.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &cover_url.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &body.description.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &disc_count.map(|value| D1Type::Integer(i32::try_from(value).unwrap_or(i32::MAX))).unwrap_or(D1Type::Null),
            &body.volume.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &body.parent_book_id.map(|value| id_type(value, "parent_book_id")).transpose().map_err(|error| worker::Error::from(error.to_string()))?.unwrap_or(D1Type::Null),
            &D1Type::Text(body.media_type.as_deref().unwrap_or("cd")),
            &body.series_id.map(|value| id_type(value, "series_id")).transpose().map_err(|error| worker::Error::from(error.to_string()))?.unwrap_or(D1Type::Null),
        ])
        .map_err(db_error)?
        .first::<CdRow>(None)
        .await
        .map_err(db_error)?;
    let Some(cd) = row else {
        return error_response(AppError::Internal("cd insert returned no row".into()));
    };
    let cd_id = id_type(cd.id, "cd id").map_err(|error| worker::Error::from(error.to_string()))?;
    if let Some(tracks) = tracks {
        for track in tracks {
            let disc_number = D1Type::Integer(
                i32::try_from(track.disc_number.unwrap_or(1).max(1)).unwrap_or(i32::MAX),
            );
            let track_number =
                D1Type::Integer(i32::try_from(track.track_number).unwrap_or(i32::MAX));
            let duration = track
                .duration
                .as_deref()
                .map(D1Type::Text)
                .unwrap_or(D1Type::Null);
            let row = db
                .prepare(
                    "INSERT INTO tracks (cd_id,disc_number,track_number,title,duration)
                     VALUES (?,?,?,?,?) RETURNING id",
                )
                .bind_refs([
                    &cd_id,
                    &disc_number,
                    &track_number,
                    &D1Type::Text(&track.title),
                    &duration,
                ])
                .map_err(db_error)?
                .first::<serde_json::Value>(None)
                .await
                .map_err(db_error)?;
            let track_id = row
                .and_then(|value| value.get("id").and_then(|value| value.as_i64()))
                .ok_or_else(|| worker::Error::from("track insert returned no row"))?;
            insert_track_metadata(&db, track_id, &track).await?;
        }
    }
    if let Some(metadata) = body.metadata {
        ctx.d1("DB")?.prepare("INSERT INTO cd_metadata (cd_id,year,genre,composer,isrc) VALUES (?,?,?,?,?) ON CONFLICT(cd_id) DO UPDATE SET year=excluded.year,genre=excluded.genre,composer=excluded.composer,isrc=excluded.isrc").bind_refs([&cd_id, &metadata.year.map(|value| D1Type::Integer(i32::try_from(value).unwrap_or(i32::MAX))).unwrap_or(D1Type::Null), &metadata.genre.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null), &metadata.composer.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null), &metadata.isrc.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null)]).map_err(db_error)?.run().await.map_err(db_error)?;
    }
    if let Some(author_ids) = body.author_ids {
        for author_id in author_ids {
            let author_id = id_type(author_id, "author id")
                .map_err(|error| worker::Error::from(error.to_string()))?;
            ctx.d1("DB")?
                .prepare("INSERT OR IGNORE INTO cd_authors (cd_id, author_id) VALUES (?,?)")
                .bind_refs([&cd_id, &author_id])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
        }
    }
    if let Some(grand_series_id) = body.grand_series_id.filter(|value| *value > 0) {
        let grand_series_id = id_type(grand_series_id, "grand_series id")
            .map_err(|error| worker::Error::from(error.to_string()))?;
        db.prepare(
            "INSERT OR IGNORE INTO grand_series_items (grand_series_id, item_type, item_id)
             VALUES (?, 'cd', ?)",
        )
        .bind_refs([&grand_series_id, &cd_id])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    }
    let value = with_children(&db, cd).await?;
    Response::from_json(&serde_json::json!({"cd": value, "source": source}))
        .map(|response| response.with_status(201))
}

const CD_MUTABLE_COLUMNS: &[&str] = &[
    "jan",
    "title",
    "artist",
    "publisher",
    "label",
    "catalog_number",
    "publish_date",
    "cover_url",
    "disc_count",
    "volume",
    "parent_book_id",
    "media_type",
    "series_id",
];

fn value_for<'a>(value: Option<&'a serde_json::Value>) -> D1Type<'a> {
    match value {
        Some(value) if value.is_null() => D1Type::Null,
        Some(value) => value
            .as_str()
            .map(D1Type::Text)
            .or_else(|| {
                value
                    .as_i64()
                    .and_then(|value| i32::try_from(value).ok())
                    .map(D1Type::Integer)
            })
            .unwrap_or(D1Type::Null),
        None => D1Type::Null,
    }
}

async fn update_metadata(
    db: &worker::D1Database,
    cd_id: &D1Type<'_>,
    body: &serde_json::Value,
) -> Result<()> {
    let mut assignments = Vec::new();
    for key in ["year", "genre", "composer", "isrc"] {
        if body.get(key).is_some() {
            assignments.push(format!("{key} = excluded.{key}"));
        }
    }
    if assignments.is_empty() {
        return Ok(());
    }
    let year = value_for(body.get("year"));
    let genre = value_for(body.get("genre"));
    let composer = value_for(body.get("composer"));
    let isrc = value_for(body.get("isrc"));
    let values = [cd_id, &year, &genre, &composer, &isrc];
    let sql = format!(
        "INSERT INTO cd_metadata (cd_id,year,genre,composer,isrc)
         VALUES (?,?,?,?,?)
         ON CONFLICT(cd_id) DO UPDATE SET {}, updated_at=CURRENT_TIMESTAMP",
        assignments.join(", ")
    );
    db.prepare(&sql)
        .bind_refs(values)
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(())
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match req.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(error) => return Ok(bad_request(format!("invalid JSON: {error}"))),
    };
    let db = ctx.d1("DB")?;
    if row(&db, &id).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    let mut assignments = Vec::new();
    let mut values = Vec::new();
    for column in CD_MUTABLE_COLUMNS {
        if let Some(value) = body.get(*column) {
            assignments.push(format!("{column} = ?"));
            values.push(value_for(Some(value)));
        }
    }
    if !assignments.is_empty() {
        assignments.push("updated_at = CURRENT_TIMESTAMP".to_string());
        let sql = format!("UPDATE cds SET {} WHERE id = ?", assignments.join(", "));
        let mut refs = values.iter().collect::<Vec<_>>();
        refs.push(&id);
        db.prepare(&sql)
            .bind_refs(refs)
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
    }
    if let Some(metadata) = body.get("metadata") {
        if let Some(metadata) = metadata.as_object() {
            update_metadata(&db, &id, &serde_json::Value::Object(metadata.clone())).await?;
        }
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let result = ctx
        .d1("DB")?
        .prepare("DELETE FROM cds WHERE id = ?")
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

pub async fn add_author(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    author_mutation(
        &ctx,
        "INSERT OR IGNORE INTO cd_authors (cd_id,author_id) VALUES (?,?)",
        false,
    )
    .await
}
pub async fn remove_author(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    author_mutation(
        &ctx,
        "DELETE FROM cd_authors WHERE cd_id = ? AND author_id = ?",
        false,
    )
    .await
}

async fn author_mutation(ctx: &RouteContext<()>, sql: &str, _order: bool) -> Result<Response> {
    let cd = match parse_id(ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let author = match parse_id(ctx, "author_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let cd = match id_type(cd, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let author = match id_type(author, "author id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    ctx.d1("DB")?
        .prepare(sql)
        .bind_refs([&cd, &author])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}

pub async fn update_author_order(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cd = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let author = match parse_id(&ctx, "author_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let body = match req.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(error) => return Ok(bad_request(format!("invalid JSON: {error}"))),
    };
    let order = body
        .get("sort_order")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let cd = match id_type(cd, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let author = match id_type(author, "author id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    ctx.d1("DB")?
        .prepare("UPDATE cd_authors SET sort_order = ? WHERE cd_id = ? AND author_id = ?")
        .bind_refs([
            &D1Type::Integer(i32::try_from(order).unwrap_or(i32::MAX)),
            &cd,
            &author,
        ])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}

pub async fn add_authors_from_names(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw_cd = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let body = match parse_json::<serde_json::Value>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let names = body
        .get("names")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let cd = match id_type(raw_cd, "cd id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    for value in names {
        let Some(name) = value
            .as_str()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        let name_value = D1Type::Text(name);
        let db = ctx.d1("DB")?;
        let author = db
            .prepare("SELECT id FROM authors WHERE name = ?")
            .bind_refs(&name_value)
            .map_err(db_error)?
            .first::<serde_json::Value>(None)
            .await
            .map_err(db_error)?;
        let author = match author.and_then(|value| value.get("id").and_then(|value| value.as_i64()))
        {
            Some(author) => author,
            None => db
                .prepare("INSERT INTO authors (name) VALUES (?) RETURNING id")
                .bind_refs(&name_value)
                .map_err(db_error)?
                .first::<serde_json::Value>(None)
                .await
                .map_err(db_error)?
                .and_then(|value| value.get("id").and_then(|value| value.as_i64()))
                .unwrap_or_default(),
        };
        let author =
            id_type(author, "author id").map_err(|error| worker::Error::from(error.to_string()))?;
        ctx.d1("DB")?
            .prepare("INSERT OR IGNORE INTO cd_authors (cd_id,author_id) VALUES (?,?)")
            .bind_refs([&cd, &author])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
    }
    Ok(Response::empty()?.with_status(204))
}
