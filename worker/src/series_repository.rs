use dantalian::{
    application::error::AppError, domain::series::Series,
    ports::series_repository::SeriesRepository,
};
use worker::{D1Database, D1Type};

pub struct D1SeriesRepository {
    db: D1Database,
}

impl D1SeriesRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    fn map_error(error: worker::Error) -> AppError {
        AppError::Database(error.to_string())
    }

    fn bind_id(id: i64) -> Result<D1Type<'static>, AppError> {
        let id = i32::try_from(id)
            .map_err(|_| AppError::Validation("Series id is out of range".to_string()))?;
        Ok(D1Type::Integer(id))
    }
}

impl SeriesRepository for D1SeriesRepository {
    async fn list(&self) -> Result<Vec<Series>, AppError> {
        let result = self
            .db
            .prepare("SELECT id, name FROM series ORDER BY name")
            .all()
            .await
            .map_err(Self::map_error)?;
        result.results::<Series>().map_err(Self::map_error)
    }

    async fn create(&self, name: &str) -> Result<Series, AppError> {
        let name = D1Type::Text(name);
        self.db
            .prepare("INSERT INTO series (name) VALUES (?) RETURNING id, name")
            .bind_refs(&name)
            .map_err(Self::map_error)?
            .first::<Series>(None)
            .await
            .map_err(Self::map_error)?
            .ok_or_else(|| AppError::Database("series insert returned no row".to_string()))
    }

    async fn rename(&self, id: i64, name: &str) -> Result<(), AppError> {
        let name = D1Type::Text(name);
        let id = Self::bind_id(id)?;
        let result = self
            .db
            .prepare("UPDATE series SET name = ? WHERE id = ?")
            .bind_refs([&name, &id])
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
            .prepare("DELETE FROM series WHERE id = ?")
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
