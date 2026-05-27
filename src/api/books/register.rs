use crate::AppState;
use crate::db::{BookAuthor, BookWithAuthors, NewBook};
use crate::external;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use super::ApiError;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub isbn: String,
    pub media_type: Option<String>,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub book: BookWithAuthors,
    pub source: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), ApiError> {
    let isbn = req.isbn.replace('-', "").replace(' ', "");
    if isbn.len() != 13 && isbn.len() != 10 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "ISBN must be 10 or 13 digits"})),
        ));
    }

    if let Ok(Some(existing)) = state.db.find_by_isbn(&isbn) {
        let authors = state.db.get_book_authors(existing.id).unwrap_or_default();
        return Ok((
            StatusCode::OK,
            Json(RegisterResponse {
                book: BookWithAuthors {
                    book: existing,
                    authors,
                    copies_count: 0,
                    lent_count: 0,
                },
                source: "cache".to_string(),
            }),
        ));
    }

    let new_book = external::lookup_isbn(&state.client, &isbn, &state.images_dir)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": e})),
            )
        })?;

    let Some(mut new_book) = new_book else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Book not found for this ISBN"})),
        ));
    };

    let source = if new_book.cover_url.is_some() {
        "amazon"
    } else {
        "ndl"
    }
    .to_string();

    if let Some(ref mt) = req.media_type {
        new_book.media_type = Some(mt.clone());
    }

    let book = state.db.insert_book(&new_book).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    let mut authors = Vec::new();
    for a in &new_book.authors {
        let aid = state
            .db
            .insert_author(a.ndl_id.as_deref(), &a.name, a.transcription.as_deref())
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;
        state.db.add_book_author(book.id, aid).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        let full = state.db.get_author_by_id(aid).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        if let Some(author) = full {
            authors.push(BookAuthor {
                id: author.id,
                ndl_id: author.ndl_id,
                name: author.name,
                transcription: author.transcription,
                sort_order: authors.len() as i64,
            });
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            book: BookWithAuthors { book, authors, copies_count: 0, lent_count: 0 },
            source,
        }),
    ))
}

#[derive(Deserialize)]
pub struct IsdnRegisterRequest {
    pub isdn: String,
}

pub async fn isdn_register(
    State(state): State<AppState>,
    Json(req): Json<IsdnRegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), ApiError> {
    let isdn: String = req.isdn.chars().filter(|c| c.is_ascii_digit()).collect();
    if isdn.len() != 13 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "ISDN must be 13 digits"})),
        ));
    }

    if let Ok(Some(existing)) = state.db.find_by_isdn(&isdn) {
        let authors = state.db.get_book_authors(existing.id).unwrap_or_default();
        return Ok((
            StatusCode::OK,
            Json(RegisterResponse {
                book: BookWithAuthors {
                    book: existing,
                    authors,
                    copies_count: 0,
                    lent_count: 0,
                },
                source: "cache".to_string(),
            }),
        ));
    }

    let new_book = external::lookup_isdn(&state.client, &isdn)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": e})),
            )
        })?;

    let Some(new_book) = new_book else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Book not found for this ISDN"})),
        ));
    };

    let book = state.db.insert_book(&new_book).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            book: BookWithAuthors {
                book,
                authors: Vec::new(),
                copies_count: 0,
                lent_count: 0,
            },
            source: "isdn".to_string(),
        }),
    ))
}

#[derive(Deserialize)]
pub struct ManualRegisterRequest {
    pub isbn: Option<String>,
    pub isdn: Option<String>,
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub alternative: Option<String>,
    pub alternative_transcription: Option<String>,
    pub volume: Option<String>,
    pub volume_transcription: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub jpno: Option<String>,
    pub ndl_url: Option<String>,
    pub series_id: Option<Option<i64>>,
    pub series_number: Option<i64>,
    pub grand_series_id: Option<Option<i64>>,
    pub author_ids: Option<Vec<i64>>,
    pub isdn_region: Option<String>,
    pub isdn_class: Option<String>,
    pub isdn_type: Option<String>,
    pub isdn_rating_gender: Option<String>,
    pub isdn_rating_age: Option<String>,
    pub isdn_genre_code: Option<String>,
    pub isdn_genre_name: Option<String>,
    pub isdn_genre_user: Option<String>,
    pub isdn_c_code: Option<String>,
    pub isdn_author: Option<String>,
    pub isdn_shape: Option<String>,
    pub isdn_contents: Option<String>,
    pub isdn_barcode2: Option<String>,
    pub isdn_sample_image_url: Option<String>,
    pub isdn_useroption: Option<String>,
    pub isdn_external_links: Option<String>,
}

pub async fn manual_register(
    State(state): State<AppState>,
    Json(req): Json<ManualRegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), ApiError> {
    let isbn = req
        .isbn
        .map(|s| s.trim().replace(['-', ' '], ""))
        .filter(|s| !s.is_empty());
    let isdn = req
        .isdn
        .map(|s| s.trim().replace(['-', ' '], ""))
        .filter(|s| !s.is_empty());
    if isbn.is_none() && isdn.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "ISBN or ISDN is required"})),
        ));
    }
    if req.title.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Title is required"})),
        ));
    }

    if let Some(ref isdn_val) = isdn {
        if let Ok(Some(existing)) = state.db.find_by_isdn(isdn_val) {
            let authors = state.db.get_book_authors(existing.id).unwrap_or_default();
            return Ok((
                StatusCode::OK,
                Json(RegisterResponse {
                    book: BookWithAuthors {
                        book: existing,
                        authors,
                        copies_count: 0,
                        lent_count: 0,
                    },
                    source: "cache".to_string(),
                }),
            ));
        }
    }
    if let Some(ref isbn_val) = isbn {
        if let Ok(Some(existing)) = state.db.find_by_isbn(isbn_val) {
            let authors = state.db.get_book_authors(existing.id).unwrap_or_default();
            return Ok((
                StatusCode::OK,
                Json(RegisterResponse {
                    book: BookWithAuthors {
                        book: existing,
                        authors,
                        copies_count: 0,
                        lent_count: 0,
                    },
                    source: "cache".to_string(),
                }),
            ));
        }
    }

    let new_book = NewBook {
        isbn,
        isdn,
        title: req.title.trim().to_string(),
        publisher: req
            .publisher
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        publish_date: req
            .publish_date
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        cover_url: None,
        description: req
            .description
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        title_transcription: req
            .title_transcription
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        series_title: req
            .series_title
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        series_title_transcription: req
            .series_title_transcription
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        alternative: req
            .alternative
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        alternative_transcription: req
            .alternative_transcription
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        volume: req
            .volume
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        volume_transcription: req
            .volume_transcription
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        price: req
            .price
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        extent: req
            .extent
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        jpno: req
            .jpno
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        ndl_url: req
            .ndl_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        authors: Vec::new(),
        isdn_region: req.isdn_region.filter(|s| !s.is_empty()),
        isdn_class: req.isdn_class.filter(|s| !s.is_empty()),
        isdn_type: req.isdn_type.filter(|s| !s.is_empty()),
        isdn_rating_gender: req.isdn_rating_gender.filter(|s| !s.is_empty()),
        isdn_rating_age: req.isdn_rating_age.filter(|s| !s.is_empty()),
        isdn_genre_code: req.isdn_genre_code.filter(|s| !s.is_empty()),
        isdn_genre_name: req.isdn_genre_name.filter(|s| !s.is_empty()),
        isdn_genre_user: req.isdn_genre_user.filter(|s| !s.is_empty()),
        isdn_c_code: req.isdn_c_code.filter(|s| !s.is_empty()),
        isdn_author: req.isdn_author.filter(|s| !s.is_empty()),
        isdn_shape: req.isdn_shape.filter(|s| !s.is_empty()),
        isdn_contents: req.isdn_contents.filter(|s| !s.is_empty()),
        isdn_barcode2: req.isdn_barcode2.filter(|s| !s.is_empty()),
        isdn_sample_image_url: req.isdn_sample_image_url.filter(|s| !s.is_empty()),
        isdn_useroption: req.isdn_useroption.filter(|s| !s.is_empty()),
        isdn_external_links: req.isdn_external_links.filter(|s| !s.is_empty()),
        jan: None,
        media_type: None,
        catalog_number: None,
        artist: None,
        label: None,
        disc_count: None,
        tracks: None,
    };

    let mut book = state.db.insert_book(&new_book).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    let series_id = req.series_id.unwrap_or(None);
    let series_number = req.series_number.filter(|&n| n > 0);
    let grand_series_id = req.grand_series_id.unwrap_or(None);

    if series_id.is_some() || series_number.is_some() {
        let _ = state.db.set_book_series(book.id, series_id);
    }
    if series_number.is_some() {
        let conn = state.db.0.lock().unwrap();
        let _ = conn.execute(
            "UPDATE books SET series_number = ?1 WHERE id = ?2",
            rusqlite::params![series_number, book.id],
        );
    }
    if let Some(gs_id) = grand_series_id {
        if gs_id != 0 {
            let _ = state.db.add_grand_series_item(gs_id, "book", book.id);
        }
    }
    book.series_id = series_id;
    book.series_number = series_number;

    let mut authors = Vec::new();
    if let Some(aids) = req.author_ids {
        for (i, &aid) in aids.iter().enumerate() {
            let _ = state.db.add_book_author(book.id, aid);
            let _ = state.db.update_book_author_order(book.id, aid, i as i64);
            if let Ok(Some(author)) = state.db.get_author_by_id(aid) {
                authors.push(BookAuthor {
                    id: author.id,
                    ndl_id: author.ndl_id,
                    name: author.name,
                    transcription: author.transcription,
                    sort_order: i as i64,
                });
            }
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            book: BookWithAuthors { book, authors, copies_count: 0, lent_count: 0 },
            source: "manual".to_string(),
        }),
    ))
}

#[derive(Deserialize)]
pub struct CdRegisterRequest {
    pub jan: String,
}

pub async fn cd_register(
    State(state): State<AppState>,
    Json(req): Json<CdRegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), ApiError> {
    let jan = req.jan.replace('-', "").replace(' ', "");
    if jan.len() < 8 || jan.len() > 14 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid JAN length"})),
        ));
    }

    if let Ok(Some(existing)) = state.db.find_by_jan(&jan) {
        let authors = state.db.get_book_authors(existing.id).unwrap_or_default();
        return Ok((
            StatusCode::OK,
            Json(RegisterResponse {
                book: BookWithAuthors {
                    book: existing,
                    authors,
                    copies_count: 0,
                    lent_count: 0,
                },
                source: "cache".to_string(),
            }),
        ));
    }

    let cd_info = external::lookup_cd(&state.client, &jan, &state.images_dir)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": e})),
            )
        })?;

    let cd_info = cd_info.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "CD not found for this JAN"})),
        )
    })?;

    let new_book = NewBook {
        isbn: None,
        isdn: None,
        jan: Some(jan),
        title: cd_info.title,
        publisher: cd_info.publisher,
        publish_date: cd_info.publish_date,
        cover_url: cd_info.cover_url,
        description: None,
        title_transcription: None,
        series_title: None,
        series_title_transcription: None,
        alternative: None,
        alternative_transcription: None,
        volume: None,
        volume_transcription: None,
        price: None,
        extent: None,
        jpno: None,
        ndl_url: None,
        authors: Vec::new(),
        isdn_region: None,
        isdn_class: None,
        isdn_type: None,
        isdn_rating_gender: None,
        isdn_rating_age: None,
        isdn_genre_code: None,
        isdn_genre_name: None,
        isdn_genre_user: None,
        isdn_c_code: None,
        isdn_author: None,
        isdn_shape: None,
        isdn_contents: None,
        isdn_barcode2: None,
        isdn_sample_image_url: None,
        isdn_useroption: None,
        isdn_external_links: None,
        media_type: Some("cd".to_string()),
        catalog_number: cd_info.catalog_number,
        artist: cd_info.artist,
        label: cd_info.label,
        disc_count: cd_info.disc_count,
        tracks: Some(cd_info.tracks),
    };

    let book = state.db.insert_book(&new_book).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    if let Some(ref tracks) = new_book.tracks {
        let _ = state.db.insert_tracks_batch(book.id, tracks);
    }

    let authors = Vec::new();
    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            book: BookWithAuthors {
                book,
                authors,
                copies_count: 0,
                lent_count: 0,
            },
            source: "musicbrainz".to_string(),
        }),
    ))
}
