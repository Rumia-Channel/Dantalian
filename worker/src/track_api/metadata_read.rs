use worker::{Request, Response, Result, RouteContext};

use crate::d1_read;
use crate::error::{error_response, parse_id};
use dantalian::application::error::AppError;

use super::{db_error, id_type, merge_custom_metadata};

const SELECT_METADATA: &str = "SELECT tm.*, c.id AS __cd_id, cm.cd_id AS __cd_metadata_id,
    c.title AS __album, c.publisher AS __publisher, c.label AS __label,
    cm.year AS __year, cm.genre AS __genre, cm.composer AS __composer, cm.isrc AS __isrc,
    (SELECT json_group_array(json_object('id',a.id,'ndl_id',a.ndl_id,'name',a.name,
             'transcription',a.transcription,'sort_order',a.sort_order))
     FROM (SELECT a.id,a.ndl_id,a.name,a.transcription,ta.sort_order
           FROM track_authors ta JOIN authors a ON a.id=ta.author_id
           WHERE ta.track_id=t.id ORDER BY ta.sort_order,ta.author_id) a) AS __artists
    FROM tracks t
    LEFT JOIN track_metadata tm ON tm.track_id=t.id
    LEFT JOIN cds c ON c.id=t.cd_id
    LEFT JOIN cd_metadata cm ON cm.cd_id=t.cd_id";

pub(super) async fn get(
    req: &Request,
    ctx: &RouteContext<()>,
    parent_column: &str,
) -> Result<Response> {
    let parent = match parse_id(ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let track = match parse_id(ctx, "track_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let parent_id = match id_type(parent, "parent id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let track_id = match id_type(track, "track id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let db = d1_read::read_db(req, &db)?;
    let row = db
        .prepare(format!(
            "{SELECT_METADATA} WHERE t.id=? AND t.{parent_column}=?"
        ))
        .bind_refs([&track_id, &parent_id])
        .map_err(db_error)?
        .first::<serde_json::Map<String, serde_json::Value>>(None)
        .await
        .map_err(db_error)?;
    let Some(mut row) = row else {
        return error_response(AppError::NotFound);
    };
    let has_cd = row.remove("__cd_id").is_some_and(|value| !value.is_null());
    let has_cd_metadata = row
        .remove("__cd_metadata_id")
        .is_some_and(|value| !value.is_null());
    let artists = row
        .remove("__artists")
        .and_then(|value| {
            value
                .as_str()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        })
        .unwrap_or_else(|| serde_json::json!([]));
    let fallback = [
        ("album", "__album", has_cd),
        ("publisher", "__publisher", has_cd),
        ("label", "__label", has_cd),
        ("year", "__year", has_cd_metadata),
        ("genre", "__genre", has_cd_metadata),
        ("composer", "__composer", has_cd_metadata),
        ("isrc", "__isrc", has_cd_metadata),
    ]
    .map(|(target, source, available)| (target, row.remove(source), available));
    let has_metadata = row.get("track_id").is_some_and(|value| !value.is_null());
    let mut value = if has_metadata {
        serde_json::Value::Object(row)
    } else {
        serde_json::json!({"track_id": track})
    };
    merge_custom_metadata(&mut value);
    if let Some(object) = value.as_object_mut() {
        for (key, source, available) in fallback {
            if available && object.get(key).is_none_or(serde_json::Value::is_null) {
                if let Some(source) = source {
                    object.insert(key.to_string(), source);
                }
            }
        }
        object.insert("artists".into(), artists);
        object
            .entry("album_artists")
            .or_insert_with(|| serde_json::json!([]));
    }
    Response::from_json(&value)
}
