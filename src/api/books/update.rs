use super::ApiError;
use crate::{AppState, domain::patch::deserialize_nullable, external};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

fn error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(serde_json::json!({"error":message.into()})))
}

fn database_error(error_value: rusqlite::Error) -> ApiError {
    let status = match &error_value {
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            StatusCode::CONFLICT
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StatusCode::BAD_REQUEST
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    error(status, error_value.to_string())
}

#[derive(Deserialize)]
pub struct SetSeriesRequest {
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub series_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub series_number: Option<Option<i64>>,
}

pub async fn set_series(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<SetSeriesRequest>,
) -> Result<StatusCode, ApiError> {
    let existing = state
        .db
        .find_by_id(id)
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "Book not found"))?;
    let series_id = req.series_id.unwrap_or(existing.series_id);
    let series_number = if req.series_id == Some(None) {
        None
    } else {
        req.series_number.unwrap_or(existing.series_number)
    };
    if series_id.is_some_and(|id| id <= 0) || series_number.is_some_and(|number| number <= 0) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Series id and number must be positive",
        ));
    }
    if !state
        .db
        .set_book_series(id, series_id, series_number)
        .map_err(database_error)?
    {
        return Err(error(StatusCode::NOT_FOUND, "Book not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct UpdateBookRequest {
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isbn: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub jan: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub publisher: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub publish_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub title_transcription: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub series_title: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub series_title_transcription: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub alternative: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub alternative_transcription: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub volume: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub volume_transcription: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub price: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub extent: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub jpno: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub ndl_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub series_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub series_number: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub grand_series_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub media_type: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub catalog_number: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub artist: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub label: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub disc_count: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_region: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_class: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_type: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_rating_gender: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_rating_age: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_genre_code: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_genre_name: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_genre_user: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_c_code: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_author: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_shape: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_contents: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_barcode2: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_sample_image_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_useroption: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub isdn_external_links: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub reading_status: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub storage_location_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub label_id: Option<Option<i64>>,
}

pub async fn update_book(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateBookRequest>,
) -> Result<StatusCode, ApiError> {
    let existing = state
        .db
        .find_by_id(id)
        .map_err(database_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "Book not found"))?;
    // Every nullable field follows the same absent/present/null rule, without copying old strings.
    macro_rules! resolve_fields {
        ($($field:ident),* $(,)?) => { $(let $field = req.$field.unwrap_or(existing.$field);)* };
    }
    resolve_fields!(
        isbn,
        isdn,
        jan,
        publisher,
        publish_date,
        description,
        title_transcription,
        series_title,
        series_title_transcription,
        alternative,
        alternative_transcription,
        volume,
        volume_transcription,
        price,
        extent,
        jpno,
        ndl_url,
        series_id,
        series_number,
        media_type,
        catalog_number,
        artist,
        label,
        disc_count,
        isdn_region,
        isdn_class,
        isdn_type,
        isdn_rating_gender,
        isdn_rating_age,
        isdn_genre_code,
        isdn_genre_name,
        isdn_genre_user,
        isdn_c_code,
        isdn_author,
        isdn_shape,
        isdn_contents,
        isdn_barcode2,
        isdn_sample_image_url,
        isdn_useroption,
        isdn_external_links,
        reading_status,
        storage_location_id,
        label_id
    );
    let series_number = if req.series_id == Some(None) {
        None
    } else {
        series_number
    };
    let isbn = isbn
        .map(|s| s.trim().replace(['-', ' '], ""))
        .filter(|s| !s.is_empty());
    let isdn = isdn
        .map(|s| s.trim().replace(['-', ' '], ""))
        .filter(|s| !s.is_empty());
    if isbn.is_none() && isdn.is_none() {
        return Err(error(StatusCode::BAD_REQUEST, "ISBN or ISDN is required"));
    }
    if req.title.trim().is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "Title is required"));
    }
    if [
        series_id,
        storage_location_id,
        label_id,
        req.grand_series_id.flatten(),
    ]
    .into_iter()
    .flatten()
    .any(|id| id <= 0)
        || series_number.is_some_and(|number| number <= 0)
        || disc_count.is_some_and(|count| count <= 0)
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Ids and counts must be positive",
        ));
    }
    let publish_date = external::normalize_publish_date_input(publish_date.as_deref())
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    let updated = state
        .db
        .update_book(
            id,
            isbn.as_deref(),
            isdn.as_deref(),
            jan.as_deref(),
            req.title.trim(),
            publisher.as_deref(),
            publish_date.as_deref(),
            description.as_deref(),
            title_transcription.as_deref(),
            series_title.as_deref(),
            series_title_transcription.as_deref(),
            alternative.as_deref(),
            alternative_transcription.as_deref(),
            volume.as_deref(),
            volume_transcription.as_deref(),
            price.as_deref(),
            extent.as_deref(),
            jpno.as_deref(),
            ndl_url.as_deref(),
            series_id,
            series_number,
            isdn_region.as_deref(),
            isdn_class.as_deref(),
            isdn_type.as_deref(),
            isdn_rating_gender.as_deref(),
            isdn_rating_age.as_deref(),
            isdn_genre_code.as_deref(),
            isdn_genre_name.as_deref(),
            isdn_genre_user.as_deref(),
            isdn_c_code.as_deref(),
            isdn_author.as_deref(),
            isdn_shape.as_deref(),
            isdn_contents.as_deref(),
            isdn_barcode2.as_deref(),
            isdn_sample_image_url.as_deref(),
            isdn_useroption.as_deref(),
            isdn_external_links.as_deref(),
            media_type.as_deref(),
            catalog_number.as_deref(),
            artist.as_deref(),
            label.as_deref(),
            disc_count,
            reading_status.as_deref(),
            storage_location_id,
            label_id,
            req.grand_series_id,
        )
        .map_err(database_error)?;
    if !updated {
        return Err(error(StatusCode::NOT_FOUND, "Book not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nullable_updates_distinguish_missing_clear_and_value() {
        let missing: UpdateBookRequest =
            serde_json::from_value(serde_json::json!({"title":"Book"})).unwrap();
        assert_eq!(missing.jan, None);
        assert_eq!(missing.series_id, None);
        let clear: UpdateBookRequest = serde_json::from_value(
            serde_json::json!({"title":"Book","jan":null,"artist":null,"series_id":null}),
        )
        .unwrap();
        assert_eq!(clear.jan, Some(None));
        assert_eq!(clear.artist, Some(None));
        assert_eq!(clear.series_id, Some(None));
        let set: SetSeriesRequest =
            serde_json::from_value(serde_json::json!({"series_id":7,"series_number":null}))
                .unwrap();
        assert_eq!(set.series_id, Some(Some(7)));
        assert_eq!(set.series_number, Some(None));
    }
}
