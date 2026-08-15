use serde::{Deserialize, Serialize};
use worker::{D1Type, Request, Response, Result, RouteContext};

use crate::{
    amazon_api,
    error::{bad_request, error_response, parse_id, parse_json},
    external_api,
};
use dantalian::application::error::AppError;

const BOOK_COLUMNS: &str = "id,isbn,isdn,jan,title,publisher,publish_date,cover_url,description,series_id,series_number,media_type,title_transcription,series_title,series_title_transcription,alternative,alternative_transcription,volume,volume_transcription,price,extent,jpno,ndl_url,isdn_region,isdn_class,isdn_type,isdn_rating_gender,isdn_rating_age,isdn_genre_code,isdn_genre_name,isdn_genre_user,isdn_c_code,isdn_author,isdn_shape,isdn_contents,isdn_barcode2,isdn_sample_image_url,isdn_useroption,isdn_external_links,catalog_number,artist,label,disc_count,epub_file_hash,epub_file_name,reading_status,storage_location_id,label_id,created_at,updated_at";

#[derive(Debug, Serialize, Deserialize)]
struct BookRow {
    id: i64,
    isbn: Option<String>,
    isdn: Option<String>,
    jan: Option<String>,
    title: String,
    publisher: Option<String>,
    publish_date: Option<String>,
    cover_url: Option<String>,
    description: Option<String>,
    series_id: Option<i64>,
    series_number: Option<i64>,
    media_type: Option<String>,
    title_transcription: Option<String>,
    series_title: Option<String>,
    series_title_transcription: Option<String>,
    alternative: Option<String>,
    alternative_transcription: Option<String>,
    volume: Option<String>,
    volume_transcription: Option<String>,
    price: Option<String>,
    extent: Option<String>,
    jpno: Option<String>,
    ndl_url: Option<String>,
    isdn_region: Option<String>,
    isdn_class: Option<String>,
    isdn_type: Option<String>,
    isdn_rating_gender: Option<String>,
    isdn_rating_age: Option<String>,
    isdn_genre_code: Option<String>,
    isdn_genre_name: Option<String>,
    isdn_genre_user: Option<String>,
    isdn_c_code: Option<String>,
    isdn_author: Option<String>,
    isdn_shape: Option<String>,
    isdn_contents: Option<String>,
    isdn_barcode2: Option<String>,
    isdn_sample_image_url: Option<String>,
    isdn_useroption: Option<String>,
    isdn_external_links: Option<String>,
    catalog_number: Option<String>,
    artist: Option<String>,
    label: Option<String>,
    disc_count: Option<i64>,
    epub_file_hash: Option<String>,
    epub_file_name: Option<String>,
    reading_status: Option<String>,
    storage_location_id: Option<i64>,
    label_id: Option<i64>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorOrder {
    sort_order: Option<i64>,
}
#[derive(Debug, Serialize, Deserialize)]
struct BookSummaryRow {
    id: i64,
    isbn: Option<String>,
    isdn: Option<String>,
    jan: Option<String>,
    title: String,
    publisher: Option<String>,
    publish_date: Option<String>,
    cover_url: Option<String>,
    description: Option<String>,
    series_id: Option<i64>,
    series_number: Option<i64>,
    media_type: Option<String>,
    copies_count: i64,
    lent_count: i64,
    primary_author_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct BooksPage {
    items: Vec<serde_json::Value>,
    next_cursor: Option<String>,
}

const BOOK_CURSOR_MASK: i64 = 0x5A17_C0DE;

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

async fn load(db: &worker::D1Database, raw: i64) -> Result<Option<BookRow>> {
    let id = id_type(raw, "book id").map_err(|error| worker::Error::from(error.to_string()))?;
    db.prepare(&format!("SELECT {BOOK_COLUMNS} FROM books WHERE id = ?"))
        .bind_refs(&id)
        .map_err(db_error)?
        .first::<BookRow>(None)
        .await
        .map_err(db_error)
}
async fn output(db: &worker::D1Database, book: BookRow) -> Result<serde_json::Value> {
    let id = id_type(book.id, "book id").map_err(|error| worker::Error::from(error.to_string()))?;
    let authors = db
        .prepare(
            "SELECT a.id,a.ndl_id,a.name,a.transcription,ba.sort_order
             FROM authors a
             JOIN book_authors ba ON ba.author_id=a.id
             WHERE ba.book_id=?
             ORDER BY ba.sort_order,ba.author_id",
        )
        .bind_refs(&id)
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<serde_json::Value>()
        .map_err(db_error)?;
    let counts = db
        .prepare(
            "SELECT COUNT(*) AS copies_count,
                    COALESCE(SUM(CASE WHEN lh.id IS NOT NULL
                                      AND lh.returned_date IS NULL
                                      THEN 1 ELSE 0 END), 0) AS lent_count
             FROM copies c
             LEFT JOIN lending_history lh
               ON lh.copy_id = c.id AND lh.returned_date IS NULL
             WHERE c.book_id = ?",
        )
        .bind_refs(&id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .unwrap_or_default();
    let mut value =
        serde_json::to_value(book).map_err(|error| worker::Error::from(error.to_string()))?;
    if let Some(object) = value.as_object_mut() {
        object.insert("authors".into(), serde_json::Value::Array(authors));
        object.insert(
            "copies_count".into(),
            counts
                .get("copies_count")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::from(0)),
        );
        object.insert(
            "lent_count".into(),
            counts
                .get("lent_count")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::from(0)),
        );
    }
    Ok(value)
}

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (limit, cursor) = match list_query(&req) {
        Ok(query) => query,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    const SUMMARY_SELECT: &str = "SELECT b.id,b.isbn,b.isdn,b.jan,b.title,b.publisher,b.publish_date,b.cover_url,b.description,b.series_id,b.series_number,b.media_type,
        COUNT(DISTINCT c.id) AS copies_count,
        COUNT(DISTINCT CASE WHEN lh.id IS NOT NULL THEN c.id END) AS lent_count,
        (SELECT a.name FROM authors a
         JOIN book_authors ba ON ba.author_id = a.id
         WHERE ba.book_id = b.id
         ORDER BY ba.sort_order,ba.author_id
         LIMIT 1) AS primary_author_name
        FROM books b
        LEFT JOIN copies c ON c.book_id = b.id
        LEFT JOIN lending_history lh ON lh.copy_id = c.id AND lh.returned_date IS NULL";
    let rows = if let Some(cursor) = cursor {
        db.prepare(&format!(
            "{SUMMARY_SELECT} WHERE b.id < ? GROUP BY b.id ORDER BY b.id DESC LIMIT ?"
        ))
        .bind_refs([
            &D1Type::Integer(cursor),
            &D1Type::Integer(i32::try_from(limit + 1).unwrap_or(101)),
        ])
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<BookSummaryRow>()
        .map_err(db_error)?
    } else {
        db.prepare(&format!(
            "{SUMMARY_SELECT} GROUP BY b.id ORDER BY b.id DESC LIMIT ?"
        ))
        .bind_refs(&D1Type::Integer(i32::try_from(limit + 1).unwrap_or(101)))
        .map_err(db_error)?
        .all()
        .await
        .map_err(db_error)?
        .results::<BookSummaryRow>()
        .map_err(db_error)?
    };
    let has_more = rows.len() > limit;
    let mut rows = rows;
    if has_more {
        rows.truncate(limit);
    }
    let next_cursor = rows
        .last()
        .map(|row| encode_cursor(row.id))
        .filter(|_| has_more);
    let items = rows
        .into_iter()
        .map(|row| {
            let mut value = serde_json::to_value(row)
                .map_err(|error| worker::Error::from(error.to_string()))?;
            if let Some(object) = value.as_object_mut() {
                if object
                    .get("media_type")
                    .is_some_and(serde_json::Value::is_null)
                {
                    object.insert(
                        "media_type".into(),
                        serde_json::Value::String("book".into()),
                    );
                }
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>>>()?;
    Response::from_json(&BooksPage { items, next_cursor })
}

fn list_query(req: &Request) -> std::result::Result<(usize, Option<i32>), Response> {
    let query = req.url().map_err(|_| bad_request("invalid request URL"))?;
    let params = query.query_pairs().into_owned().collect::<Vec<_>>();
    let limit = match params.iter().find(|(key, _)| key == "limit") {
        Some((_, value)) => value
            .parse::<usize>()
            .ok()
            .filter(|value| (1..=100).contains(value))
            .ok_or_else(|| bad_request("limit must be between 1 and 100"))?,
        None => 50,
    };
    let cursor = match params.iter().find(|(key, _)| key == "cursor") {
        Some((_, value)) => Some(decode_cursor(value)?),
        None => None,
    };
    Ok((limit, cursor))
}

fn encode_cursor(id: i64) -> String {
    format!("c{:x}", id ^ BOOK_CURSOR_MASK)
}

fn decode_cursor(value: &str) -> std::result::Result<i32, Response> {
    let encoded = value
        .strip_prefix('c')
        .filter(|value| !value.is_empty() && value.len() <= 16)
        .ok_or_else(|| bad_request("invalid cursor"))?;
    let encoded = i64::from_str_radix(encoded, 16).map_err(|_| bad_request("invalid cursor"))?;
    let id = encoded ^ BOOK_CURSOR_MASK;
    let id = i32::try_from(id).map_err(|_| bad_request("invalid cursor"))?;
    if id <= 0 {
        return Err(bad_request("invalid cursor"));
    }
    Ok(id)
}

fn has_cover_url(body: &serde_json::Value) -> bool {
    body.get("cover_url")
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty())
}

fn set_cover_url(body: &mut serde_json::Value, file_name: String) {
    if let Some(object) = body.as_object_mut() {
        object.insert(
            "cover_url".to_string(),
            serde_json::Value::String(file_name),
        );
    }
}

fn set_amazon_metadata(
    body: &mut serde_json::Value,
    info: &dantalian::amazon::AmazonInfo,
    metadata_verified: bool,
) {
    if !metadata_verified {
        return;
    }
    let Some(object) = body.as_object_mut() else {
        return;
    };
    if let Some(description) = info
        .description
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        object.insert(
            "description".to_string(),
            serde_json::Value::String(description.to_string()),
        );
    }
    if let Some(publish_date) = info
        .publish_date
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        object.insert(
            "publish_date".to_string(),
            serde_json::Value::String(publish_date.to_string()),
        );
    }
}

pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    match load(&ctx.d1("DB")?, raw).await? {
        Some(book) => Response::from_json(&output(&ctx.d1("DB")?, book).await?),
        None => error_response(AppError::NotFound),
    }
}

async fn register(
    mut req: Request,
    ctx: RouteContext<()>,
    isdn: bool,
    manual: bool,
) -> Result<Response> {
    let body = match req.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(error) => return Ok(bad_request(format!("invalid JSON: {error}"))),
    };
    let manual = manual
        || body
            .get("title")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty());
    let has_isbn = body
        .get("isbn")
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty());
    let identifier_key = if isdn || (!has_isbn && body.get("isdn").is_some()) {
        "isdn"
    } else {
        "isbn"
    };
    let identifier = body
        .get(identifier_key)
        .and_then(|value| value.as_str())
        .map(normalize_identifier)
        .map(|value| {
            if isdn {
                value.chars().filter(char::is_ascii_digit).collect()
            } else {
                value
            }
        })
        .filter(|value| !value.is_empty());
    if identifier.is_none() {
        return Ok(bad_request(if manual {
            "ISBN or ISDN is required"
        } else if isdn {
            "ISDN is required"
        } else {
            "ISBN is required"
        }));
    }
    if isdn && identifier.as_ref().is_some_and(|value| value.len() != 13) {
        return Ok(bad_request("ISDN must be 13 digits"));
    }
    if !isdn
        && !manual
        && identifier
            .as_ref()
            .is_some_and(|value| value.len() != 10 && value.len() != 13)
    {
        return Ok(bad_request("ISBN must be 10 or 13 digits"));
    }
    if manual
        && !body
            .get("title")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(bad_request("Title is required"));
    }
    let db = ctx.d1("DB")?;
    if let Some(identifier) = &identifier {
        let sql = format!("SELECT {BOOK_COLUMNS} FROM books WHERE {identifier_key} = ?");
        let existing = db
            .prepare(&sql)
            .bind_refs(&D1Type::Text(identifier))
            .map_err(db_error)?
            .first::<BookRow>(None)
            .await
            .map_err(db_error)?;
        if let Some(book) = existing {
            return Response::from_json(&serde_json::json!({
                "book": output(&db, book).await?,
                "source": "cache"
            }));
        }
    }
    let request_cover_supplied = has_cover_url(&body);
    let mut body = body;
    let mut source = if manual {
        "manual"
    } else if isdn {
        "isdn"
    } else {
        "ndl"
    };
    if !manual {
        let lookup = if isdn {
            external_api::lookup_isdn(identifier.as_deref().unwrap_or_default()).await
        } else {
            external_api::lookup_isbn(identifier.as_deref().unwrap_or_default()).await
        };
        match lookup {
            Ok(Some(book)) => {
                merge_ndl_book(&mut body, &book);
                if !isdn && !request_cover_supplied {
                    let amazon_search_terms = ["title", "alternative"]
                        .into_iter()
                        .filter_map(|field| body.get(field).and_then(|value| value.as_str()))
                        .collect::<Vec<_>>();
                    match amazon_api::lookup_amazon_metadata(
                        identifier.as_deref().unwrap_or_default(),
                        &amazon_search_terms,
                    )
                    .await
                    {
                        Ok(Some(metadata)) => {
                            let amazon_api::AmazonMetadata {
                                info,
                                metadata_verified,
                                cover,
                            } = metadata;
                            set_amazon_metadata(&mut body, &info, metadata_verified);
                            if let Some(cover) = cover {
                                match amazon_api::persist_cover(&ctx, &cover).await {
                                    Ok(file_name) => {
                                        set_cover_url(&mut body, file_name);
                                        source = "amazon";
                                    }
                                    Err(error) => {
                                        worker::console_error!(
                                            "Amazon cover storage failed; keeping NDL metadata: {error}"
                                        );
                                    }
                                }
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            worker::console_error!(
                                "Amazon metadata lookup failed; keeping NDL metadata: {error}"
                            );
                        }
                    }
                }
            }
            Ok(None) => {
                return error_response(AppError::NotFound);
            }
            Err(error) => {
                return Response::from_json(&serde_json::json!({
                    "error": error.to_string()
                }))
                .map(|response| response.with_status(502));
            }
        }
    }
    let isbn = normalized_identifier(body.get("isbn"));
    let isdn_value = normalized_identifier(body.get("isdn"));
    let title = body
        .get("title")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            format!(
                "{} {}",
                if isdn { "ISDN" } else { "ISBN" },
                identifier.clone().unwrap_or_default()
            )
        });
    let row = db
        .prepare(
            "INSERT INTO books (isbn,isdn,title,media_type,created_at,updated_at)
             VALUES (?,?,?,?,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP) RETURNING id",
        )
        .bind_refs([
            &isbn.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &isdn_value
                .as_deref()
                .map(D1Type::Text)
                .unwrap_or(D1Type::Null),
            &D1Type::Text(&title),
            &D1Type::Text(
                body.get("media_type")
                    .and_then(|value| value.as_str())
                    .unwrap_or("book"),
            ),
        ])
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?;
    let Some(id) = row.and_then(|value| value.get("id").and_then(|value| value.as_i64())) else {
        return error_response(AppError::Internal("book insert returned no row".into()));
    };
    let book_id = id_type(id, "book id").map_err(|error| worker::Error::from(error.to_string()))?;
    db.prepare("UPDATE books SET isbn = ?, isdn = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind_refs([
            &isbn.as_deref().map(D1Type::Text).unwrap_or(D1Type::Null),
            &isdn_value
                .as_deref()
                .map(D1Type::Text)
                .unwrap_or(D1Type::Null),
            &book_id,
        ])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    apply_book_fields(&db, &book_id, &body).await?;
    insert_ndl_authors(&db, &book_id, &body).await?;
    if let Some(author_ids) = body.get("author_ids").and_then(|value| value.as_array()) {
        for author_id in author_ids.iter().filter_map(|value| value.as_i64()) {
            let author_id = id_type(author_id, "author id")
                .map_err(|error| worker::Error::from(error.to_string()))?;
            db.prepare("INSERT OR IGNORE INTO book_authors (book_id, author_id) VALUES (?, ?)")
                .bind_refs([&book_id, &author_id])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
        }
    }
    if let Some(grand_series_id) = body
        .get("grand_series_id")
        .and_then(|value| value.as_i64())
        .filter(|value| *value > 0)
    {
        let grand_series_id = id_type(grand_series_id, "grand_series id")
            .map_err(|error| worker::Error::from(error.to_string()))?;
        db.prepare(
            "INSERT OR IGNORE INTO grand_series_items (grand_series_id, item_type, item_id)
             VALUES (?, 'book', ?)",
        )
        .bind_refs([&grand_series_id, &book_id])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    }
    let book = load(&db, id)
        .await?
        .ok_or_else(|| worker::Error::from("book insert returned no row"))?;
    Response::from_json(&serde_json::json!({
        "book": output(&db, book).await?,
        "source": source
    }))
    .map(|response| response.with_status(201))
}

pub async fn register_book(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    register(req, ctx, false, false).await
}
pub async fn register_isdn(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    register(req, ctx, true, false).await
}
pub async fn register_manual(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    register(req, ctx, false, true).await
}

fn merge_ndl_book(body: &mut serde_json::Value, book: &external_api::NdlBook) {
    let Some(object) = body.as_object_mut() else {
        return;
    };
    macro_rules! set_text {
        ($field:literal, $value:expr) => {
            if !object.get($field).is_some_and(|value| !value.is_null()) {
                if let Some(value) = $value.as_deref().filter(|value| !value.is_empty()) {
                    object.insert(
                        $field.to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
            }
        };
    }
    if !object.get("title").is_some_and(|value| !value.is_null()) && !book.title.is_empty() {
        object.insert(
            "title".to_string(),
            serde_json::Value::String(book.title.clone()),
        );
    }
    set_text!("publisher", book.publisher);
    set_text!("publish_date", book.publish_date);
    set_text!("description", book.description);
    set_text!("title_transcription", book.title_transcription);
    set_text!("series_title", book.series_title);
    set_text!(
        "series_title_transcription",
        book.series_title_transcription
    );
    set_text!("alternative", book.alternative);
    set_text!("alternative_transcription", book.alternative_transcription);
    set_text!("volume", book.volume);
    set_text!("volume_transcription", book.volume_transcription);
    set_text!("price", book.price);
    set_text!("extent", book.extent);
    set_text!("jpno", book.jpno);
    set_text!("cover_url", book.cover_url);
    set_text!("isdn_region", book.isdn_region);
    set_text!("isdn_class", book.isdn_class);
    set_text!("isdn_type", book.isdn_type);
    set_text!("isdn_rating_gender", book.isdn_rating_gender);
    set_text!("isdn_rating_age", book.isdn_rating_age);
    set_text!("isdn_genre_code", book.isdn_genre_code);
    set_text!("isdn_genre_name", book.isdn_genre_name);
    set_text!("isdn_genre_user", book.isdn_genre_user);
    set_text!("isdn_c_code", book.isdn_c_code);
    set_text!("isdn_author", book.isdn_author);
    set_text!("isdn_shape", book.isdn_shape);
    set_text!("isdn_contents", book.isdn_contents);
    set_text!("isdn_barcode2", book.isdn_barcode2);
    set_text!("isdn_sample_image_url", book.isdn_sample_image_url);
    set_text!("isdn_useroption", book.isdn_useroption);
    set_text!("isdn_external_links", book.isdn_external_links);
    set_text!("ndl_url", book.ndl_url);
    if !book.authors.is_empty() && !object.contains_key("_ndl_authors") {
        object.insert(
            "_ndl_authors".to_string(),
            serde_json::Value::Array(
                book.authors
                    .iter()
                    .map(|author| {
                        serde_json::json!({
                            "ndl_id": author.ndl_id,
                            "name": author.name,
                            "transcription": author.transcription,
                        })
                    })
                    .collect(),
            ),
        );
    }
}

async fn insert_ndl_authors(
    db: &worker::D1Database,
    book_id: &D1Type<'_>,
    body: &serde_json::Value,
) -> Result<()> {
    let Some(authors) = body.get("_ndl_authors").and_then(|value| value.as_array()) else {
        return Ok(());
    };
    for author in authors {
        let Some(name) = author
            .get("name")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        let ndl_id = author.get("ndl_id").and_then(|value| value.as_str());
        let transcription = author.get("transcription").and_then(|value| value.as_str());
        let author_id = if let Some(ndl_id) = ndl_id {
            db.prepare(
                "INSERT OR IGNORE INTO authors (ndl_id, name, transcription) VALUES (?, ?, ?)",
            )
            .bind_refs([
                &D1Type::Text(ndl_id),
                &D1Type::Text(name),
                &transcription.map(D1Type::Text).unwrap_or(D1Type::Null),
            ])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
            db.prepare("SELECT id FROM authors WHERE ndl_id = ?")
                .bind_refs(&D1Type::Text(ndl_id))
                .map_err(db_error)?
                .first::<serde_json::Value>(None)
                .await
                .map_err(db_error)?
                .and_then(|row| row.get("id").and_then(|value| value.as_i64()))
        } else {
            db.prepare("INSERT INTO authors (name, transcription) VALUES (?, ?) RETURNING id")
                .bind_refs([
                    &D1Type::Text(name),
                    &transcription.map(D1Type::Text).unwrap_or(D1Type::Null),
                ])
                .map_err(db_error)?
                .first::<serde_json::Value>(None)
                .await
                .map_err(db_error)?
                .and_then(|row| row.get("id").and_then(|value| value.as_i64()))
        };
        let Some(author_id) = author_id else {
            continue;
        };
        let author_id = id_type(author_id, "author id")
            .map_err(|error| worker::Error::from(error.to_string()))?;
        db.prepare("INSERT OR IGNORE INTO book_authors (book_id, author_id) VALUES (?, ?)")
            .bind_refs([book_id, &author_id])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
    }
    Ok(())
}

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
];

async fn apply_book_fields(
    db: &worker::D1Database,
    id: &D1Type<'_>,
    body: &serde_json::Value,
) -> Result<()> {
    let mut assignments = Vec::new();
    let mut values = Vec::new();
    for column in BOOK_MUTABLE_COLUMNS {
        if let Some(value) = body.get(*column) {
            assignments.push(format!("{column} = ?"));
            values.push(value_for(Some(value)));
        }
    }
    if assignments.is_empty() {
        return Ok(());
    }
    let sql = format!(
        "UPDATE books SET {}, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        assignments.join(", ")
    );
    let mut refs = values.iter().collect::<Vec<_>>();
    refs.push(id);
    db.prepare(&sql)
        .bind_refs(refs)
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn update_grand_series(
    db: &worker::D1Database,
    book_id: &D1Type<'_>,
    body: &serde_json::Value,
) -> Result<()> {
    let Some(value) = body.get("grand_series_id") else {
        return Ok(());
    };
    db.prepare("DELETE FROM grand_series_items WHERE item_type = 'book' AND item_id = ?")
        .bind_refs(book_id)
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    let Some(grand_series_id) = value.as_i64().filter(|value| *value > 0) else {
        return Ok(());
    };
    let grand_series_id = id_type(grand_series_id, "grand_series id")
        .map_err(|error| worker::Error::from(error.to_string()))?;
    db.prepare(
        "INSERT OR IGNORE INTO grand_series_items (grand_series_id, item_type, item_id)
         VALUES (?, 'book', ?)",
    )
    .bind_refs([&grand_series_id, book_id])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    Ok(())
}

fn normalize_identifier(value: &str) -> String {
    value.replace(['-', ' ', '　'], "")
}

fn normalized_identifier(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|value| value.as_str())
        .map(normalize_identifier)
        .filter(|value| !value.is_empty())
}

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
    if load(&db, raw).await?.is_none() {
        return error_response(AppError::NotFound);
    }
    let body = match req.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(error) => return Ok(bad_request(format!("invalid JSON: {error}"))),
    };
    apply_book_fields(&db, &id, &body).await?;
    update_grand_series(&db, &id, &body).await?;
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
    let db = ctx.d1("DB")?;
    let current = match load(&db, raw).await? {
        Some(book) => book,
        None => return error_response(AppError::NotFound),
    };
    let series = body
        .get("series_id")
        .map(|value| value_for(Some(value)))
        .unwrap_or_else(|| {
            current
                .series_id
                .map(|value| D1Type::Integer(i32::try_from(value).unwrap_or(i32::MAX)))
                .unwrap_or(D1Type::Null)
        });
    let number = body
        .get("series_number")
        .map(|value| value_for(Some(value)))
        .unwrap_or_else(|| {
            current
                .series_number
                .map(|value| D1Type::Integer(i32::try_from(value).unwrap_or(i32::MAX)))
                .unwrap_or(D1Type::Null)
        });
    db.prepare(
        "UPDATE books SET series_id=?,series_number=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",
    )
    .bind_refs([&series, &number, &id])
    .map_err(db_error)?
    .run()
    .await
    .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let raw = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let id = match id_type(raw, "book id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let old_hash = db
        .prepare("SELECT epub_file_hash FROM books WHERE id = ?")
        .bind_refs(&id)
        .map_err(db_error)?
        .first::<serde_json::Value>(None)
        .await
        .map_err(db_error)?
        .and_then(|row| {
            row.get("epub_file_hash")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        });
    let result = db
        .prepare("DELETE FROM books WHERE id = ?")
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
    if let Some(hash) = old_hash {
        let still_referenced = db
            .prepare("SELECT 1 FROM books WHERE epub_file_hash = ? LIMIT 1")
            .bind_refs(&D1Type::Text(&hash))
            .map_err(db_error)?
            .first::<serde_json::Value>(None)
            .await
            .map_err(db_error)?
            .is_some();
        if !still_referenced {
            db.prepare("DELETE FROM object_uploads WHERE object_kind = 'epub' AND object_key = ?")
                .bind_refs(&D1Type::Text(&hash))
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
        }
    }
    Ok(Response::empty()?.with_status(204))
}

pub async fn author_add(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    author_mutation(
        &ctx,
        "INSERT OR IGNORE INTO book_authors (book_id,author_id) VALUES (?,?)",
    )
    .await
}
pub async fn author_remove(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    author_mutation(
        &ctx,
        "DELETE FROM book_authors WHERE book_id=? AND author_id=?",
    )
    .await
}
async fn author_mutation(ctx: &RouteContext<()>, sql: &str) -> Result<Response> {
    let book = match parse_id(ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let author = match parse_id(ctx, "author_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let book = match id_type(book, "book id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let author = match id_type(author, "author id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    ctx.d1("DB")?
        .prepare(sql)
        .bind_refs([&book, &author])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}
pub async fn author_order(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let book = match parse_id(&ctx, "id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let author = match parse_id(&ctx, "author_id") {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let body = match parse_json::<AuthorOrder>(&mut req).await {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let book = match id_type(book, "book id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let author = match id_type(author, "author id") {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    ctx.d1("DB")?
        .prepare("UPDATE book_authors SET sort_order=? WHERE book_id=? AND author_id=?")
        .bind_refs([
            &D1Type::Integer(i32::try_from(body.sort_order.unwrap_or(0)).unwrap_or(i32::MAX)),
            &book,
            &author,
        ])
        .map_err(db_error)?
        .run()
        .await
        .map_err(db_error)?;
    Ok(Response::empty()?.with_status(204))
}

#[cfg(test)]
mod tests {
    use super::{BOOK_MUTABLE_COLUMNS, set_amazon_metadata};
    use dantalian::amazon::AmazonInfo;
    use serde_json::json;
    #[test]
    fn book_field_updates_persist_cover_urls() {
        assert!(BOOK_MUTABLE_COLUMNS.contains(&"cover_url"));
    }

    #[test]
    fn amazon_metadata_overrides_ndl_description_and_date() {
        let mut body = json!({
            "description": "NDL description",
            "publish_date": "2020-01-01"
        });
        let info = AmazonInfo {
            description: Some("Amazon description".to_string()),
            publish_date: Some("2021-02-03".to_string()),
            ..AmazonInfo::default()
        };
        set_amazon_metadata(&mut body, &info, true);
        assert_eq!(body["description"], "Amazon description");
        assert_eq!(body["publish_date"], "2021-02-03");

        set_amazon_metadata(&mut body, &info, false);
        assert_eq!(body["description"], "Amazon description");
    }
}
