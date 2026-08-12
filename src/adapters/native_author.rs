use crate::{
    application::error::AppError, db::Db, domain::author::Author,
    ports::author_repository::AuthorRepository,
};

#[derive(Clone)]
pub struct NativeAuthorRepository {
    db: Db,
}

impl NativeAuthorRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl AuthorRepository for NativeAuthorRepository {
    async fn list(&self) -> Result<Vec<Author>, AppError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.list_authors()
                .map(|authors| authors.into_iter().map(Into::into).collect())
                .map_err(|error| AppError::Database(error.to_string()))
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn get(&self, id: i64) -> Result<Author, AppError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.get_author_by_id(id)
                .map_err(|error| AppError::Database(error.to_string()))?
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
                .map_err(|error| AppError::Database(error.to_string()))
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
        .map_err(|error| AppError::Database(error.to_string()))?;
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
            .map_err(|error| AppError::Database(error.to_string()))?;
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
