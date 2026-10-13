use super::Db;
use crate::application::error::AppError;
use rusqlite::{ErrorCode, params, types::Value};

#[derive(PartialEq, Eq)]
enum FieldKind {
    Text,
    Integer,
    Real,
}

fn map_error(error: rusqlite::Error) -> AppError {
    if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
        AppError::Validation(error.to_string())
    } else {
        AppError::Database(error.to_string())
    }
}

impl Db {
    pub fn patch_track_metadata(
        &self,
        track_id: i64,
        mut body: serde_json::Value,
    ) -> Result<(), AppError> {
        let object = body
            .as_object_mut()
            .ok_or_else(|| AppError::Validation("Expected a JSON object".into()))?;
        let artists = object
            .remove("artists")
            .map(serde_json::from_value::<Vec<i64>>)
            .transpose()
            .map_err(|error| AppError::Validation(format!("Invalid artists: {error}")))?;
        if artists
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| *id <= 0))
        {
            return Err(AppError::Validation("Artist ids must be positive".into()));
        }
        let mut fields = Vec::with_capacity(object.len().min(24) + 1);
        fields.push(("track_id", Value::Integer(track_id)));
        use FieldKind::{Integer, Real, Text};
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
            ("raw_size_bytes", Integer),
        ] {
            let Some(value) = object.remove(column) else {
                continue;
            };
            let value = match value {
                serde_json::Value::Null => Value::Null,
                serde_json::Value::String(text) if kind == Text => Value::Text(text),
                serde_json::Value::Number(number) if kind == Integer && number.is_i64() => {
                    Value::Integer(number.as_i64().unwrap())
                }
                serde_json::Value::Number(number) if kind == Real => {
                    Value::Real(number.as_f64().unwrap())
                }
                _ => return Err(AppError::Validation(format!("Invalid {column}"))),
            };
            fields.push((column, value));
        }
        let mut conn = self.0.lock();
        let tx = conn.transaction().map_err(map_error)?;
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
                sql.push_str(column);
                sql.push_str("=excluded.");
                sql.push_str(column);
                sql.push(',');
            }
            sql.push_str("updated_at=CURRENT_TIMESTAMP");
            tx.execute(
                &sql,
                rusqlite::params_from_iter(fields.iter().map(|(_, value)| value)),
            )
            .map_err(map_error)?;
        } else if artists.is_some() {
            tx.execute(
                "INSERT INTO track_metadata(track_id) VALUES (?1) ON CONFLICT(track_id) DO NOTHING",
                params![track_id],
            )
            .map_err(map_error)?;
        }
        if let Some(artists) = artists {
            tx.execute(
                "DELETE FROM track_authors WHERE track_id=?1",
                params![track_id],
            )
            .map_err(map_error)?;
            let mut insert = tx
                .prepare(
                    "INSERT INTO track_authors(track_id,author_id,sort_order) VALUES (?1,?2,?3)",
                )
                .map_err(map_error)?;
            for (index, author_id) in artists.into_iter().enumerate() {
                insert
                    .execute(params![track_id, author_id, index as i64])
                    .map_err(map_error)?;
            }
        }
        tx.commit().map_err(map_error)
    }
}
