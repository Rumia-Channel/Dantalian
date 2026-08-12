use dantalian::{
    application::error::AppError, domain::author::Author,
    ports::author_repository::AuthorRepository,
};
use worker::{D1Database, D1Type};

pub struct D1AuthorRepository {
    db: D1Database,
}

impl D1AuthorRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    fn map_error(error: worker::Error) -> AppError {
        AppError::Database(error.to_string())
    }

    fn bind_id(id: i64) -> Result<D1Type<'static>, AppError> {
        let id = i32::try_from(id)
            .map_err(|_| AppError::Validation("Author id is out of range".to_string()))?;
        Ok(D1Type::Integer(id))
    }
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
}
