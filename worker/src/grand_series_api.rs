use dantalian::application::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use worker::{D1Type, Request, Response, Result, RouteContext};

use crate::d1_read;
use crate::error::{error_response, parse_id, parse_json};

#[derive(Debug, Deserialize)]
struct CreateRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AddItemRequest {
    item_type: String,
    item_id: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct GrandSeries {
    id: i64,
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Item {
    item_type: String,
    item_id: i64,
    name: String,
}

#[derive(Debug, Serialize)]
struct GrandSeriesWithItems {
    id: i64,
    name: String,
    items: Vec<Item>,
}

fn bind_id(id: i64, label: &str) -> std::result::Result<D1Type<'static>, AppError> {
    let id =
        i32::try_from(id).map_err(|_| AppError::Validation(format!("{label} is out of range")))?;
    if id <= 0 {
        return Err(AppError::Validation(format!("{label} must be positive")));
    }
    Ok(D1Type::Integer(id))
}

fn map_db(error: worker::Error) -> worker::Error {
    error
}

fn validate_name(name: &str) -> std::result::Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".to_string()));
    }
    Ok(name.to_string())
}

#[derive(Debug, Deserialize)]
struct ItemRow {
    grand_series_id: i64,
    item_type: String,
    item_id: i64,
    name: String,
}

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DB")?;
    let db = d1_read::read_db(&req, &db)?;
    let rows = db
        .prepare("SELECT id, name FROM grand_series ORDER BY name, id")
        .all()
        .await
        .map_err(map_db)?
        .results::<GrandSeries>()
        .map_err(map_db)?;
    // Load every item in one query instead of one query per grand series.
    let item_rows = db
        .prepare(
            "SELECT gsi.grand_series_id, gsi.item_type, gsi.item_id,
                    CASE gsi.item_type
                      WHEN 'series' THEN s.name
                      WHEN 'book' THEN b.title
                      WHEN 'cd' THEN c.title
                    END AS name
             FROM grand_series_items gsi
             LEFT JOIN series s ON gsi.item_type = 'series' AND s.id = gsi.item_id
             LEFT JOIN books b ON gsi.item_type = 'book' AND b.id = gsi.item_id
             LEFT JOIN cds c ON gsi.item_type = 'cd' AND c.id = gsi.item_id
             WHERE CASE gsi.item_type WHEN 'series' THEN s.id WHEN 'book' THEN b.id WHEN 'cd' THEN c.id END IS NOT NULL
             ORDER BY gsi.grand_series_id, gsi.item_type, gsi.item_id",
        )
        .all()
        .await
        .map_err(map_db)?
        .results::<ItemRow>()
        .map_err(map_db)?;
    let mut grouped: HashMap<i64, Vec<Item>> = HashMap::new();
    for row in item_rows {
        grouped.entry(row.grand_series_id).or_default().push(Item {
            item_type: row.item_type,
            item_id: row.item_id,
            name: row.name,
        });
    }
    let result = rows
        .into_iter()
        .map(|grand_series| GrandSeriesWithItems {
            id: grand_series.id,
            name: grand_series.name,
            items: grouped.remove(&grand_series.id).unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    Response::from_json(&result)
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CreateRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let name = match validate_name(&request.name) {
        Ok(name) => name,
        Err(error) => return error_response(error),
    };
    let name = D1Type::Text(&name);
    let row = ctx
        .d1("DB")?
        .prepare("INSERT INTO grand_series (name) VALUES (?) RETURNING id, name")
        .bind_refs(&name)
        .map_err(map_db)?
        .first::<GrandSeries>(None)
        .await
        .map_err(map_db)?
        .ok_or_else(|| AppError::Database("grand series insert returned no row".to_string()));
    match row {
        Ok(value) => Response::from_json(&value).map(|response| response.with_status(201)),
        Err(error) => error_response(error),
    }
}

pub async fn rename(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let request = match parse_json::<CreateRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let name = match validate_name(&request.name) {
        Ok(name) => name,
        Err(error) => return error_response(error),
    };
    let id = match bind_id(id, "grand series id") {
        Ok(id) => id,
        Err(error) => return error_response(error),
    };
    let name = D1Type::Text(&name);
    let result = ctx
        .d1("DB")?
        .prepare("UPDATE grand_series SET name = ? WHERE id = ?")
        .bind_refs([&name, &id])
        .map_err(map_db)?
        .run()
        .await
        .map_err(map_db)?;
    if result
        .meta()
        .map_err(map_db)?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        == 0
    {
        return error_response(AppError::NotFound);
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let id = match bind_id(id, "grand series id") {
        Ok(id) => id,
        Err(error) => return error_response(error),
    };
    let result = ctx
        .d1("DB")?
        .prepare("DELETE FROM grand_series WHERE id = ?")
        .bind_refs(&id)
        .map_err(map_db)?
        .run()
        .await
        .map_err(map_db)?;
    if result
        .meta()
        .map_err(map_db)?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        == 0
    {
        return error_response(AppError::NotFound);
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn add_item(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let grand_series_id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let request = match parse_json::<AddItemRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    if !matches!(request.item_type.as_str(), "series" | "book" | "cd") || request.item_id <= 0 {
        return Ok(crate::error::bad_request("invalid grand series item"));
    }
    let grand_series_id = match bind_id(grand_series_id, "grand series id") {
        Ok(id) => id,
        Err(error) => return error_response(error),
    };
    let item_id = match bind_id(request.item_id, "item id") {
        Ok(id) => id,
        Err(error) => return error_response(error),
    };
    let item_type = D1Type::Text(&request.item_type);
    let db = ctx.d1("DB")?;
    let table = match request.item_type.as_str() {
        "series" => "series",
        "book" => "books",
        _ => "cds",
    };
    let valid = db
        .prepare(format!(
            "SELECT 1 WHERE EXISTS(SELECT 1 FROM grand_series WHERE id=?1)
         AND EXISTS(SELECT 1 FROM {table} WHERE id=?2)"
        ))
        .bind_refs([&grand_series_id, &item_id])
        .map_err(map_db)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(map_db)?
        .is_some();
    if !valid {
        return error_response(AppError::NotFound);
    }
    let result = ctx
        .d1("DB")?
        .prepare(
            "INSERT OR IGNORE INTO grand_series_items (grand_series_id, item_type, item_id)
             VALUES (?, ?, ?)",
        )
        .bind_refs([&grand_series_id, &item_type, &item_id])
        .map_err(map_db)?
        .run()
        .await
        .map_err(map_db)?;
    if result
        .meta()
        .map_err(map_db)?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        == 0
    {
        return error_response(AppError::Conflict(
            "item already belongs to grand series".to_string(),
        ));
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn remove_item(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let grand_series_id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let item_id = match parse_id(&ctx, "item_id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let item_type = ctx
        .param("item_type")
        .map(|value| value.as_str())
        .unwrap_or_default();
    if !matches!(item_type, "series" | "book" | "cd") {
        return Ok(crate::error::bad_request("invalid item_type"));
    }
    let grand_series_id = match bind_id(grand_series_id, "grand series id") {
        Ok(id) => id,
        Err(error) => return error_response(error),
    };
    let item_id = match bind_id(item_id, "item id") {
        Ok(id) => id,
        Err(error) => return error_response(error),
    };
    let item_type = D1Type::Text(item_type);
    let result = ctx
        .d1("DB")?
        .prepare(
            "DELETE FROM grand_series_items
             WHERE grand_series_id = ? AND item_type = ? AND item_id = ?",
        )
        .bind_refs([&grand_series_id, &item_type, &item_id])
        .map_err(map_db)?
        .run()
        .await
        .map_err(map_db)?;
    if result
        .meta()
        .map_err(map_db)?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        == 0
    {
        return error_response(AppError::NotFound);
    }
    Ok(Response::empty()?.with_status(204))
}
