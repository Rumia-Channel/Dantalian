use crate::{
    application::error::AppError, db::Db, domain::borrower::Borrower,
    ports::borrower_repository::BorrowerRepository,
};

#[derive(Clone)]
pub struct NativeBorrowerRepository {
    db: Db,
}

impl NativeBorrowerRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl BorrowerRepository for NativeBorrowerRepository {
    async fn list(&self) -> Result<Vec<Borrower>, AppError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.list_borrowers()
                .map(|borrowers| borrowers.into_iter().map(Into::into).collect())
                .map_err(|error| AppError::Database(error.to_string()))
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn create(&self, name: &str, notes: Option<&str>) -> Result<Borrower, AppError> {
        let db = self.db.clone();
        let name = name.to_string();
        let notes = notes.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            db.insert_borrower(&name, notes.as_deref())
                .map(Into::into)
                .map_err(|error| AppError::Database(error.to_string()))
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn update(
        &self,
        id: i64,
        name: Option<&str>,
        notes: Option<&str>,
    ) -> Result<(), AppError> {
        let updated = self
            .db
            .update_borrower(id, name, notes)
            .map_err(|error| AppError::Database(error.to_string()))?;
        if updated {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        let deleted = self
            .db
            .delete_borrower(id)
            .map_err(|error| AppError::Database(error.to_string()))?;
        if deleted {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
}

impl From<crate::db::Borrower> for Borrower {
    fn from(borrower: crate::db::Borrower) -> Self {
        Self {
            id: borrower.id,
            name: borrower.name,
            notes: borrower.notes,
        }
    }
}
