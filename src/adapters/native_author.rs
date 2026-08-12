use crate::{
    application::error::AppError, db::Db, domain::author::Author,
    ports::author_repository::AuthorRepository,
};
use rusqlite::{Error as RusqliteError, ErrorCode};

#[derive(Clone)]
pub struct NativeAuthorRepository {
    db: Db,
}

impl NativeAuthorRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

fn map_db_error(error: RusqliteError) -> AppError {
    match error {
        RusqliteError::SqliteFailure(code, _) if code.code == ErrorCode::ConstraintViolation => {
            AppError::Conflict("Author ndl_id already exists".to_string())
        }
        other => AppError::Database(other.to_string()),
    }
}

impl AuthorRepository for NativeAuthorRepository {
    async fn list(&self) -> Result<Vec<Author>, AppError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.list_authors()
                .map(|authors| authors.into_iter().map(Into::into).collect())
                .map_err(map_db_error)
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn get(&self, id: i64) -> Result<Author, AppError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.get_author_by_id(id)
                .map_err(map_db_error)?
                .map(Into::into)
                .ok_or(AppError::NotFound)
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn create(
        &self,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> Result<Author, AppError> {
        let db = self.db.clone();
        let name = name.to_string();
        let transcription = transcription.map(str::to_string);
        let ndl_id = ndl_id.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            db.create_author(&name, transcription.as_deref(), ndl_id.as_deref())
                .map(Into::into)
                .map_err(map_db_error)
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn update(
        &self,
        id: i64,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> Result<(), AppError> {
        let db = self.db.clone();
        let name = name.to_string();
        let transcription = transcription.map(str::to_string);
        let ndl_id = ndl_id.map(str::to_string);
        let updated = tokio::task::spawn_blocking(move || {
            db.update_author(id, &name, transcription.as_deref(), ndl_id.as_deref())
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
        .map_err(map_db_error)?;
        if updated {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        let db = self.db.clone();
        let deleted = tokio::task::spawn_blocking(move || db.delete_author(id))
            .await
            .map_err(|error| AppError::Internal(error.to_string()))?
            .map_err(map_db_error)?;
        if deleted {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
}

impl From<crate::db::Author> for Author {
    fn from(author: crate::db::Author) -> Self {
        Self {
            id: author.id,
            ndl_id: author.ndl_id,
            name: author.name,
            transcription: author.transcription,
        }
    }
}
