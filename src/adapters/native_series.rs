use crate::{
    application::error::AppError, db::Db, domain::series::Series,
    ports::series_repository::SeriesRepository,
};

#[derive(Clone)]
pub struct NativeSeriesRepository {
    db: Db,
}

impl NativeSeriesRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SeriesRepository for NativeSeriesRepository {
    async fn list(&self) -> Result<Vec<Series>, AppError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.list_series()
                .map(|series| series.into_iter().map(Into::into).collect())
                .map_err(|error| AppError::Database(error.to_string()))
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn create(&self, name: &str) -> Result<Series, AppError> {
        let db = self.db.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            db.create_series(&name)
                .map(Into::into)
                .map_err(|error| AppError::Database(error.to_string()))
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn rename(&self, id: i64, name: &str) -> Result<(), AppError> {
        let db = self.db.clone();
        let name = name.to_string();
        let updated = tokio::task::spawn_blocking(move || db.rename_series(id, &name))
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
        let deleted = tokio::task::spawn_blocking(move || db.delete_series(id))
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

impl From<crate::db::Series> for Series {
    fn from(series: crate::db::Series) -> Self {
        Self {
            id: series.id,
            name: series.name,
        }
    }
}
