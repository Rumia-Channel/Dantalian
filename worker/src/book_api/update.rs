use super::{db_error, id_type, load, value_for};
use crate::d1_read::ReadDb;
use crate::error::{bad_request, error_response, parse_id, parse_json};
use dantalian::application::error::AppError;
use worker::{D1Database, D1PreparedStatement, D1Type, Request, Response, Result, RouteContext};

const BOOK_MUTABLE_COLUMNS: &[&str] = &[
    "isbn",
    "isdn",
    "jan",
    "title",
    "publisher",
    "publish_date",
    "description",
    "cover_url",
    "series_id",
    "series_number",
    "media_type",
    "title_transcription",
    "series_title",
    "series_title_transcription",
    "alternative",
    "alternative_transcription",
    "volume",
    "volume_transcription",
    "price",
    "extent",
    "jpno",
    "ndl_url",
    "reading_status",
    "storage_location_id",
    "label_id",
    "disc_count",
    "artist",
    "label",
    "catalog_number",
    "isdn_region",
    "isdn_class",
    "isdn_type",
    "isdn_rating_gender",
    "isdn_rating_age",
    "isdn_genre_code",
    "isdn_genre_name",
    "isdn_genre_user",
    "isdn_c_code",
    "isdn_author",
    "isdn_shape",
    "isdn_contents",
    "isdn_barcode2",
    "isdn_sample_image_url",
    "isdn_useroption",
    "isdn_external_links",
];

pub(super) fn fields_statement(
    db: &D1Database,
    id: &D1Type<'_>,
    body: &serde_json::Value,
) -> Result<Option<D1PreparedStatement>> {
    let mut sql = String::from("UPDATE books SET ");
    let mut values = Vec::with_capacity(body.as_object().map_or(0, |object| object.len()));
    for column in BOOK_MUTABLE_COLUMNS {
        if let Some(value) = body.get(*column) {
            sql.push_str(column);
            sql.push_str("=?, ");
            values.push(value_for(Some(value)));
        }
    }
    if values.is_empty() {
        return Ok(None);
    }
    sql.push_str("updated_at=CURRENT_TIMESTAMP WHERE id=?");
    db.prepare(&sql)
        .bind_refs(values.iter().chain(std::iter::once(id)))
        .map(Some)
        .map_err(db_error)
}

fn validate_fields(body: &serde_json::Value) -> std::result::Result<(), AppError> {
    if !body.is_object() {
        return Err(AppError::Validation("Expected a JSON object".into()));
    }
    for column in BOOK_MUTABLE_COLUMNS
        .iter()
        .copied()
        .chain(std::iter::once("grand_series_id"))
    {
        let Some(value) = body.get(column) else {
            continue;
        };
        if column == "title" && value.as_str().is_none_or(|title| title.trim().is_empty()) {
            return Err(AppError::Validation("Title is required".into()));
        }
        if value.is_null() {
            continue;
        }
        if matches!(
            column,
            "series_id"
                | "series_number"
                | "storage_location_id"
                | "label_id"
                | "disc_count"
                | "grand_series_id"
        ) {
            if value
                .as_i64()
                .is_none_or(|id| id <= 0 || id > i64::from(i32::MAX))
            {
                return Err(AppError::Validation(format!(
                    "{column} must be a positive integer"
                )));
            }
        } else if !value.is_string() {
            return Err(AppError::Validation(format!(
                "{column} must be text or null"
            )));
        }
    }
    Ok(())
}

async fn validate_references(
    db: &D1Database,
    body: &serde_json::Value,
) -> Result<Option<Response>> {
    for (column, table) in [
        ("series_id", "series"),
        ("storage_location_id", "storage_locations"),
        ("label_id", "labels"),
        ("grand_series_id", "grand_series"),
    ] {
        if let Some(raw) = body.get(column).and_then(serde_json::Value::as_i64) {
            let id = D1Type::Integer(raw as i32);
            if db
                .prepare(format!("SELECT id FROM {table} WHERE id=?"))
                .bind_refs(&id)?
                .first::<serde_json::Value>(None)
                .await?
                .is_none()
            {
                return Ok(Some(bad_request(format!("{column} not found"))));
            }
        }
    }
    Ok(None)
}

fn grand_series_statements(
    db: &D1Database,
    id: &D1Type<'_>,
    body: &serde_json::Value,
    statements: &mut Vec<D1PreparedStatement>,
) -> Result<()> {
    let Some(value) = body.get("grand_series_id") else {
        return Ok(());
    };
    statements.push(
        db.prepare("DELETE FROM grand_series_items WHERE item_type='book' AND item_id=?")
            .bind_refs(id)?,
    );
    if let Some(raw) = value.as_i64() {
        statements.push(db.prepare("INSERT INTO grand_series_items(grand_series_id,item_type,item_id) VALUES (?,'book',?)")
            .bind_refs([&D1Type::Integer(raw as i32), id])?);
    }
    Ok(())
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, "book id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    if load(&ReadDb::Primary(&db), raw).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    let mut body = match parse_json::<serde_json::Value>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    if let Err(error) = validate_fields(&body) {
        return error_response(error);
    }
    if body
        .get("series_id")
        .is_some_and(serde_json::Value::is_null)
    {
        body["series_number"] = serde_json::Value::Null;
    }
    if let Some(response) = validate_references(&db, &body).await? {
        return Ok(response);
    }
    let mut statements = Vec::with_capacity(3);
    if let Some(statement) = fields_statement(&db, &id, &body)? {
        statements.push(statement);
    }
    grand_series_statements(&db, &id, &body, &mut statements)?;
    if !statements.is_empty() {
        db.batch(statements).await.map_err(db_error)?;
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn set_series(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, "book id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let body = match parse_json::<serde_json::Value>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    if let Err(error) = validate_fields(&body) {
        return error_response(error);
    }
    let db = ctx.d1("DB")?;
    let current = match load(&ReadDb::Primary(&db), raw).await? {
        Some(book) => book,
        None => return error_response(AppError::NotFound),
    };
    if let Some(response) = validate_references(&db, &body).await? {
        return Ok(response);
    }
    let clearing = body
        .get("series_id")
        .is_some_and(serde_json::Value::is_null);
    let series = body
        .get("series_id")
        .map(|value| value_for(Some(value)))
        .unwrap_or_else(|| {
            current
                .series_id
                .map(|id| D1Type::Integer(id as i32))
                .unwrap_or(D1Type::Null)
        });
    let number = if clearing {
        D1Type::Null
    } else {
        body.get("series_number")
            .map(|value| value_for(Some(value)))
            .unwrap_or_else(|| {
                current
                    .series_number
                    .map(|number| D1Type::Integer(number as i32))
                    .unwrap_or(D1Type::Null)
            })
    };
    db.prepare(
        "UPDATE books SET series_id=?,series_number=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",
    )
    .bind_refs([&series, &number, &id])?
    .run()
    .await
    .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}
