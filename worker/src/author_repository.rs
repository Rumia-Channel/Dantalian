use dantalian::{
    application::error::AppError, domain::author::Author,
    ports::author_repository::AuthorRepository,
};
use serde::Deserialize;
use worker::{D1Database, D1Type};

#[derive(Debug, Deserialize)]
struct IdRow {
    #[serde(rename = "id")]
    _id: i32,
}

pub struct D1AuthorRepository {
    db: D1Database,
}

impl D1AuthorRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    fn map_error(error: worker::Error) -> AppError {
        let message = error.to_string();
        if message.to_ascii_lowercase().contains("constraint") {
            AppError::Conflict("Author ndl_id already exists".to_string())
        } else {
            AppError::Database(message)
        }
    }

    fn bind_id(id: i64) -> Result<D1Type<'static>, AppError> {
        let id = i32::try_from(id)
            .map_err(|_| AppError::Validation("Author id is out of range".to_string()))?;
        Ok(D1Type::Integer(id))
    }
}

/// Normalized-name author lookup shared by book/CD registration (worker
/// equivalent of native `Db::find_author_by_normalized_name`). Returns the
/// full row so callers can backfill missing ndl_id/transcription.
pub(crate) async fn find_author_by_normalized_name(
    db: &D1Database,
    name: &str,
) -> worker::Result<Option<Author>> {
    use dantalian::domain::author::normalize_author_name;
    let key = normalize_author_name(name);
    if key.is_empty() {
        return Ok(None);
    }
    let rows = db
        .prepare("SELECT id, ndl_id, name, transcription FROM authors")
        .all()
        .await
        .map_err(|error| worker::Error::from(error.to_string()))?
        .results::<Author>()
        .map_err(|error| worker::Error::from(error.to_string()))?;
    Ok(rows
        .into_iter()
        .find(|author| normalize_author_name(&author.name) == key))
}

impl AuthorRepository for D1AuthorRepository {
    async fn list(&self) -> Result<Vec<Author>, AppError> {
        let result = self
            .db
            .prepare("SELECT id, ndl_id, name, transcription FROM authors ORDER BY id")
            .all()
            .await
            .map_err(Self::map_error)?;
        result.results::<Author>().map_err(Self::map_error)
    }

    async fn get(&self, id: i64) -> Result<Author, AppError> {
        let id = Self::bind_id(id)?;
        self.db
            .prepare("SELECT id, ndl_id, name, transcription FROM authors WHERE id = ?")
            .bind_refs(&id)
            .map_err(Self::map_error)?
            .first::<Author>(None)
            .await
            .map_err(Self::map_error)?
            .ok_or(AppError::NotFound)
    }

    async fn create(
        &self,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> Result<Author, AppError> {
        if let Some(ndl_id) = ndl_id {
            let ndl_id_value = D1Type::Text(ndl_id);
            let existing = self
                .db
                .prepare("SELECT id FROM authors WHERE ndl_id = ?")
                .bind_refs(&ndl_id_value)
                .map_err(Self::map_error)?
                .first::<IdRow>(None)
                .await
                .map_err(Self::map_error)?;
            if existing.is_some() {
                return Err(AppError::Conflict(
                    "Author ndl_id already exists".to_string(),
                ));
            }
        }
        let ndl_id = ndl_id.map(D1Type::Text).unwrap_or(D1Type::Null);
        let name = D1Type::Text(name);
        let transcription = transcription.map(D1Type::Text).unwrap_or(D1Type::Null);
        self.db
            .prepare(
                "INSERT INTO authors (ndl_id, name, transcription) VALUES (?, ?, ?) RETURNING id, ndl_id, name, transcription",
            )
            .bind_refs([&ndl_id, &name, &transcription])
            .map_err(Self::map_error)?
            .first::<Author>(None)
            .await
            .map_err(Self::map_error)?
            .ok_or_else(|| AppError::Database("author insert returned no row".to_string()))
    }

    async fn update(
        &self,
        id: i64,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> Result<(), AppError> {
        let id = Self::bind_id(id)?;
        if let Some(ndl_id) = ndl_id {
            let ndl_id_value = D1Type::Text(ndl_id);
            let existing = self
                .db
                .prepare("SELECT id FROM authors WHERE ndl_id = ? AND id <> ?")
                .bind_refs([&ndl_id_value, &id])
                .map_err(Self::map_error)?
                .first::<IdRow>(None)
                .await
                .map_err(Self::map_error)?;
            if existing.is_some() {
                return Err(AppError::Conflict(
                    "Author ndl_id already exists".to_string(),
                ));
            }
        }
        let name = D1Type::Text(name);
        let transcription = transcription.map(D1Type::Text).unwrap_or(D1Type::Null);
        let ndl_id = ndl_id.map(D1Type::Text).unwrap_or(D1Type::Null);
        let result = self
            .db
            .prepare("UPDATE authors SET name = ?, transcription = ?, ndl_id = ? WHERE id = ?")
            .bind_refs([&name, &transcription, &ndl_id, &id])
            .map_err(Self::map_error)?
            .run()
            .await
            .map_err(Self::map_error)?;
        let changed = result
            .meta()
            .map_err(Self::map_error)?
            .and_then(|meta| meta.changes)
            .unwrap_or_default();
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        let id = Self::bind_id(id)?;
        let result = self
            .db
            .prepare("DELETE FROM authors WHERE id = ?")
            .bind_refs(&id)
            .map_err(Self::map_error)?
            .run()
            .await
            .map_err(Self::map_error)?;
        let changed = result
            .meta()
            .map_err(Self::map_error)?
            .and_then(|meta| meta.changes)
            .unwrap_or_default();
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn merge(
        &self,
        survivor_id: i64,
        duplicate_ids: &[i64],
    ) -> Result<usize, AppError> {
        let survivor = Self::bind_id(survivor_id)?;
        // Survivor must exist; service checks too, this keeps direct callers honest.
        let exists = self
            .db
            .prepare("SELECT id FROM authors WHERE id = ?")
            .bind_refs(&survivor)
            .map_err(Self::map_error)?
            .first::<IdRow>(None)
            .await
            .map_err(Self::map_error)?;
        if exists.is_none() {
            return Err(AppError::NotFound);
        }
        let mut merged = 0usize;
        for dupe in duplicate_ids.iter().copied().filter(|id| *id != survivor_id) {
            let dupe = Self::bind_id(dupe)?;
            for (table, entity) in [
                ("book_authors", "book_id"),
                ("cd_authors", "cd_id"),
                ("track_authors", "track_id"),
            ] {
                let move_links = format!(
                    "INSERT OR IGNORE INTO {table} ({entity}, author_id, sort_order) SELECT {entity}, ?1, MIN(sort_order) FROM {table} WHERE author_id = ?2 GROUP BY {entity}"
                );
                self.db
                    .prepare(&move_links)
                    .bind_refs([&survivor, &dupe])
                    .map_err(Self::map_error)?
                    .run()
                    .await
                    .map_err(Self::map_error)?;
                let drop_links = format!("DELETE FROM {table} WHERE author_id = ?");
                self.db
                    .prepare(&drop_links)
                    .bind_refs(&dupe)
                    .map_err(Self::map_error)?
                    .run()
                    .await
                    .map_err(Self::map_error)?;
            }
            // Backfill identity fields the survivor lacks. Read first, then
            // free the UNIQUE ndl_id slot before the survivor takes it over.
            let identity = self
                .db
                .prepare("SELECT ndl_id, transcription FROM authors WHERE id = ?")
                .bind_refs(&dupe)
                .map_err(Self::map_error)?
                .first::<serde_json::Value>(None)
                .await
                .map_err(Self::map_error)?;
            self.db
                .prepare("UPDATE authors SET ndl_id = NULL, transcription = NULL WHERE id = ?")
                .bind_refs(&dupe)
                .map_err(Self::map_error)?
                .run()
                .await
                .map_err(Self::map_error)?;
            if let Some(identity) = identity {
                let ndl = identity.get("ndl_id").and_then(|value| value.as_str());
                let transcription = identity
                    .get("transcription")
                    .and_then(|value| value.as_str());
                self.db
                    .prepare("UPDATE authors SET ndl_id = COALESCE(ndl_id, ?), transcription = COALESCE(transcription, ?) WHERE id = ?")
                    .bind_refs([
                        &ndl.map(D1Type::Text).unwrap_or(D1Type::Null),
                        &transcription.map(D1Type::Text).unwrap_or(D1Type::Null),
                        &survivor,
                    ])
                    .map_err(Self::map_error)?
                    .run()
                    .await
                    .map_err(Self::map_error)?;
            }
            self.db
                .prepare("DELETE FROM authors WHERE id = ?")
                .bind_refs(&dupe)
                .map_err(Self::map_error)?
                .run()
                .await
                .map_err(Self::map_error)?;
            merged += 1;
        }
        Ok(merged)
    }
}
