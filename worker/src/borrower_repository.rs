use dantalian::{
    application::error::AppError, domain::borrower::Borrower,
    ports::borrower_repository::BorrowerRepository,
};
use worker::{D1Database, D1Type};

pub struct D1BorrowerRepository {
    db: D1Database,
}

impl D1BorrowerRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    fn map_error(error: worker::Error) -> AppError {
        AppError::Database(error.to_string())
    }

    fn bind_id(id: i64) -> Result<D1Type<'static>, AppError> {
        let id = i32::try_from(id)
            .map_err(|_| AppError::Validation("Borrower id is out of range".to_string()))?;
        Ok(D1Type::Integer(id))
    }
}

impl BorrowerRepository for D1BorrowerRepository {
    async fn list(&self) -> Result<Vec<Borrower>, AppError> {
        let result = self
            .db
            .prepare("SELECT id, name, notes FROM borrowers ORDER BY name")
            .all()
            .await
            .map_err(Self::map_error)?;
        result.results::<Borrower>().map_err(Self::map_error)
    }

    async fn create(&self, name: &str, notes: Option<&str>) -> Result<Borrower, AppError> {
        let name = D1Type::Text(name);
        let notes = notes.map(D1Type::Text);
        self.db
            .prepare("INSERT INTO borrowers (name, notes) VALUES (?, ?) RETURNING id, name, notes")
            .bind_refs([&name, notes.as_ref().unwrap_or(&D1Type::Null)])
            .map_err(Self::map_error)?
            .first::<Borrower>(None)
            .await
            .map_err(Self::map_error)?
            .ok_or_else(|| AppError::Database("borrower insert returned no row".to_string()))
    }

    async fn update(
        &self,
        id: i64,
        name: Option<&str>,
        notes: Option<&str>,
    ) -> Result<(), AppError> {
        let name = name.map(D1Type::Text).unwrap_or(D1Type::Null);
        let notes = notes.map(D1Type::Text).unwrap_or(D1Type::Null);
        let id = Self::bind_id(id)?;
        let result = self
            .db
            .prepare("UPDATE borrowers SET name = COALESCE(?, name), notes = ? WHERE id = ?")
            .bind_refs([&name, &notes, &id])
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
            .prepare("DELETE FROM borrowers WHERE id = ?")
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
