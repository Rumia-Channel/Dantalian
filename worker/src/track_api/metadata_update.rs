use super::{custom_metadata_value, db_error, id_type, int_value, real_value, text_value};
use crate::error::{bad_request, error_response, parse_id, parse_json};
use dantalian::application::error::AppError;
use serde::Deserialize;
use worker::{D1Type, Request, Response, Result, RouteContext};

enum FieldKind {
    Text,
    Integer,
    ByteCount,
    Real,
}

pub(super) async fn put(
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
    let body = match parse_json::<serde_json::Value>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    if !body.is_object() {
        return Ok(bad_request("Expected a JSON object"));
    }
    let db = ctx.d1("DB")?;
    if db
        .prepare(format!(
            "SELECT id FROM tracks WHERE id=? AND {parent_column}=?"
        ))
        .bind_refs([&track_id, &parent_id])?
        .first::<serde_json::Value>(None)
        .await?
        .is_none()
    {
        return error_response(AppError::NotFound);
    }
    let artists = if let Some(value) = body.get("artists") {
        let Some(array) = value.as_array() else {
            return Ok(bad_request("Artists must be an array"));
        };
        if array.iter().any(|value| {
            value
                .as_i64()
                .is_none_or(|id| id <= 0 || id > i64::from(i32::MAX))
        }) {
            return Ok(bad_request("Invalid artist id"));
        }
        let json = serde_json::to_string(array)?;
        #[derive(Deserialize)]
        struct Count {
            count: usize,
        }
        let count = db.prepare("SELECT COUNT(DISTINCT a.id) AS count FROM json_each(?) r JOIN authors a ON a.id=r.value")
            .bind_refs(&D1Type::Text(&json))?.first::<Count>(None).await?;
        if count.is_none_or(|count| count.count != array.len()) {
            return Ok(bad_request("Artist ids must exist and be unique"));
        }
        Some(json)
    } else {
        None
    };
    let custom_json = match custom_metadata_value(&body) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let mut fields = Vec::with_capacity(24);
    fields.push(("track_id", track_id));
    use FieldKind::{ByteCount, Integer, Real, Text};
    for (column, kind) in [
        ("title", Text),
        ("artist", Text),
        ("album", Text),
        ("album_artist", Text),
        ("track_number", Integer),
        ("track_total", Integer),
        ("disc_number", Integer),
        ("disc_total", Integer),
        ("year", Integer),
        ("genre", Text),
        ("composer", Text),
        ("publisher", Text),
        ("label", Text),
        ("encoder", Text),
        ("comment", Text),
        ("lyrics", Text),
        ("replay_gain_track_gain_db", Real),
        ("replay_gain_track_peak", Real),
        ("replay_gain_album_gain_db", Real),
        ("replay_gain_album_peak", Real),
        ("file_type", Text),
        ("raw_size_bytes", ByteCount),
    ] {
        let Some(value) = body.get(column) else {
            continue;
        };
        let valid = value.is_null()
            || match kind {
                Text => value.is_string(),
                Integer => value
                    .as_i64()
                    .is_some_and(|number| i32::try_from(number).is_ok()),
                ByteCount => value.as_i64().is_some(),
                Real => value.is_number(),
            };
        if !valid {
            return Ok(bad_request(format!("Invalid {column}")));
        }
        let value = match kind {
            Text => text_value(&body, column),
            Integer => int_value(&body, column),
            ByteCount => value
                .as_i64()
                .map(|number| D1Type::Real(number as f64))
                .unwrap_or(D1Type::Null),
            Real => real_value(&body, column),
        };
        fields.push((column, value));
    }
    if custom_json != "{}" {
        fields.push(("custom_json", D1Type::Text(&custom_json)));
    }
    let mut statements = Vec::with_capacity(3);
    if fields.len() > 1 {
        let mut sql = String::with_capacity(2048);
        sql.push_str("INSERT INTO track_metadata(");
        for (index, (column, _)) in fields.iter().enumerate() {
            if index > 0 {
                sql.push(',');
            }
            sql.push_str(column);
        }
        sql.push_str(") VALUES(");
        for index in 0..fields.len() {
            if index > 0 {
                sql.push(',');
            }
            sql.push('?');
        }
        sql.push_str(") ON CONFLICT(track_id) DO UPDATE SET ");
        for (column, _) in fields.iter().skip(1) {
            if *column == "custom_json" {
                sql.push_str("custom_json=json_patch(COALESCE(track_metadata.custom_json,'{}'),excluded.custom_json),");
            } else {
                sql.push_str(column);
                sql.push_str("=excluded.");
                sql.push_str(column);
                sql.push(',');
            }
        }
        sql.push_str("updated_at=CURRENT_TIMESTAMP");
        statements.push(
            db.prepare(&sql)
                .bind_refs(fields.iter().map(|(_, value)| value))?,
        );
    }
    if let Some(artists) = &artists {
        statements.push(
            db.prepare("DELETE FROM track_authors WHERE track_id=?")
                .bind_refs(&D1Type::Integer(track_raw as i32))?,
        );
        statements.push(db.prepare("INSERT INTO track_authors(track_id,author_id,sort_order) SELECT ?1,value,CAST(key AS INTEGER) FROM json_each(?2)")
            .bind_refs([&D1Type::Integer(track_raw as i32), &D1Type::Text(artists)])?);
    }
    if !statements.is_empty() {
        db.batch(statements).await.map_err(db_error)?;
    }
    Ok(Response::empty()?.with_status(204))
}
