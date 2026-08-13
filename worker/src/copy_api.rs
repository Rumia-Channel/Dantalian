use serde::Deserialize;
use worker::{D1Type, Request, Response, Result, RouteContext};

use crate::error::{bad_request, error_response, parse_id, parse_json};
use dantalian::application::error::AppError;

#[derive(Debug, Deserialize)]
struct CopyRequest {
    copy_type: Option<String>,
    location: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LendingRequest {
    borrower_id: i64,
    due_date: Option<String>,
    notes: Option<String>,
}

fn id_type(id: i64, label: &str) -> std::result::Result<D1Type<'static>, AppError> {
    let id = i32::try_from(id).map_err(|_| AppError::Validation(format!("invalid {label}")))?;
    if id <= 0 {
        return Err(AppError::Validation(format!("invalid {label}")));
    }
    Ok(D1Type::Integer(id))
}

fn db_error(error: worker::Error) -> worker::Error {
    error
}

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let book_id = match parse_id(&ctx, "book_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let book_id = match id_type(book_id, "book_id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let rows = ctx
        .d1("DB")?
        .prepare(
            "SELECT c.id, c.book_id, c.copy_type, c.location, c.notes,
                    l.borrower_id, l.lent_date, l.due_date,
                    bo.name AS borrower_name
             FROM copies c
             LEFT JOIN lending_history l
               ON l.copy_id = c.id AND l.returned_date IS NULL
             LEFT JOIN borrowers bo ON bo.id = l.borrower_id
             WHERE c.book_id = ? ORDER BY c.id",
        )
        .bind_refs(&book_id)
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<serde_json::Value>()
        .map_err(db_error)?;
    Response::from_json(&rows)
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let book_id = match parse_id(&ctx, "book_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let book_id = match id_type(book_id, "book_id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let request = match parse_json::<CopyRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let copy_type = request.copy_type.unwrap_or_else(|| "physical".to_string());
    let location = request.location.unwrap_or_default();
    let notes = request.notes.unwrap_or_default();
    let values = [
        book_id,
        D1Type::Text(&copy_type),
        D1Type::Text(&location),
        D1Type::Text(&notes),
    ];
    let row = ctx
        .d1("DB")?
        .prepare(
            "INSERT INTO copies (book_id, copy_type, location, notes)
             VALUES (?, ?, ?, ?)
             RETURNING id, book_id, copy_type, location, notes",
        )
        .bind_refs([&values[0], &values[1], &values[2], &values[3]])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    match row {
        Some(row) => Response::from_json(&row).map(|response| response.with_status(201)),
        None => error_response(AppError::Database(
            "copy insert returned no row".to_string(),
        )),
    }
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(id, "id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let request = match parse_json::<CopyRequest>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let copy_type_value = request.copy_type;
    let location_value = request.location;
    let notes_value = request.notes;
    let copy_type = copy_type_value
        .as_deref()
        .map(D1Type::Text)
        .unwrap_or(D1Type::Null);
    let location = location_value
        .as_deref()
        .map(D1Type::Text)
        .unwrap_or(D1Type::Null);
    let notes = notes_value
        .as_deref()
        .map(D1Type::Text)
        .unwrap_or(D1Type::Null);
    let result = ctx
        .d1("DB")?
        .prepare(
            "UPDATE copies
             SET copy_type = COALESCE(?, copy_type),
                 location = COALESCE(?, location),
                 notes = COALESCE(?, notes)
             WHERE id = ?",
        )
        .bind_refs([&copy_type, &location, &notes, &id])
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

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(id, "id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let result = ctx
        .d1("DB")?
        .prepare("DELETE FROM copies WHERE id = ?")
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

pub async fn lend(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let copy_id = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let copy_id = match id_type(copy_id, "id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let request = match parse_json::<LendingRequest>(&mut req).await {
        Ok(response) => response,
        Err(response) => return Ok(response),
    };
    let borrower_id = match id_type(request.borrower_id, "borrower_id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let copy_exists = db
        .prepare("SELECT 1 FROM copies WHERE id = ?")
        .bind_refs(&copy_id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .is_some();
    if !copy_exists {
        return error_response(AppError::NotFound);
    }
    let borrower_exists = db
        .prepare("SELECT 1 FROM borrowers WHERE id = ?")
        .bind_refs(&borrower_id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .is_some();
    if !borrower_exists {
        return error_response(AppError::NotFound);
    }
    let active = db
        .prepare("SELECT 1 FROM lending_history WHERE copy_id = ? AND returned_date IS NULL")
        .bind_refs(&copy_id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    if active.is_some() {
        return error_response(AppError::Conflict("Copy is already lent".to_string()));
    }
    let due_date = request.due_date.unwrap_or_default();
    let notes = request.notes.unwrap_or_default();
    let lent_date = today();
    let result = db
        .prepare(
            "INSERT INTO lending_history (copy_id, borrower_id, lent_date, due_date, notes)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind_refs([
            &copy_id,
            &borrower_id,
            &D1Type::Text(&lent_date),
            &D1Type::Text(&due_date),
            &D1Type::Text(&notes),
        ])
        .map_err(db_error)?
        .run()
        .await;
    if let Err(error) = result {
        return error_response(AppError::Conflict(error.to_string()));
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn return_copy(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let copy_id = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let copy_id = match id_type(copy_id, "id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let today = today();
    let result = ctx
        .d1("DB")?
        .prepare(
            "UPDATE lending_history SET returned_date = ?
             WHERE copy_id = ? AND returned_date IS NULL",
        )
        .bind_refs([&D1Type::Text(&today), &copy_id])
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
        return Ok(bad_request("copy is not lent"));
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn history(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let copy_id = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let copy_id = match id_type(copy_id, "id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let rows = ctx
        .d1("DB")?
        .prepare(
            "SELECT l.*, b.name AS borrower_name
             FROM lending_history l
             LEFT JOIN borrowers b ON b.id = l.borrower_id
             WHERE l.copy_id = ? ORDER BY l.id DESC",
        )
        .bind_refs(&copy_id)
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<serde_json::Value>()
        .map_err(db_error)?;
    Response::from_json(&rows)
}

fn today() -> String {
    let days = (worker::Date::now().as_millis() / 86_400_000) as i64;
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_index = (5 * doy + 2) / 153;
    let day = doy - (153 * month_index + 2) / 5 + 1;
    let month = month_index + if month_index < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    format!("{year:04}-{month:02}-{day:02}")
}
