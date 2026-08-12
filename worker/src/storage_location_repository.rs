use dantalian::{
    application::error::AppError, domain::storage_location::StorageLocation,
    ports::storage_location_repository::StorageLocationRepository,
};
use worker::{D1Database, D1Type};

pub struct D1StorageLocationRepository {
    db: D1Database,
}

impl D1StorageLocationRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    fn map_error(error: worker::Error) -> AppError {
        AppError::Database(error.to_string())
    }

    fn bind_id(id: i64) -> Result<D1Type<'static>, AppError> {
        let id = i32::try_from(id)
            .map_err(|_| AppError::Validation("Storage location id is out of range".to_string()))?;
        Ok(D1Type::Integer(id))
    }
}

impl StorageLocationRepository for D1StorageLocationRepository {
    async fn list(&self) -> Result<Vec<StorageLocation>, AppError> {
        let result = self
            .db
            .prepare(
                "SELECT id, name, parent_id FROM storage_locations \
                 ORDER BY parent_id IS NOT NULL, parent_id, name",
            )
            .all()
            .await
            .map_err(Self::map_error)?;
        result.results::<StorageLocation>().map_err(Self::map_error)
    }

    async fn create(
        &self,
        name: &str,
        parent_id: Option<i64>,
    ) -> Result<StorageLocation, AppError> {
        let name = D1Type::Text(name);
        let parent_id = match parent_id {
            Some(parent_id) => Self::bind_id(parent_id)?,
            None => D1Type::Null,
        };
        self.db
            .prepare(
                "INSERT INTO storage_locations (name, parent_id) VALUES (?, ?) \
                 RETURNING id, name, parent_id",
            )
            .bind_refs([&name, &parent_id])
            .map_err(Self::map_error)?
            .first::<StorageLocation>(None)
            .await
            .map_err(Self::map_error)?
            .ok_or_else(|| {
                AppError::Database("storage location insert returned no row".to_string())
            })
    }

    async fn update(
        &self,
        id: i64,
        name: Option<&str>,
        parent_id: Option<Option<i64>>,
    ) -> Result<(), AppError> {
        let id = Self::bind_id(id)?;
        let changed = match (name, parent_id) {
            (Some(name), Some(parent_id)) => {
                let name = D1Type::Text(name);
                let parent_id = match parent_id {
                    Some(parent_id) => Self::bind_id(parent_id)?,
                    None => D1Type::Null,
                };
                self.db
                    .prepare("UPDATE storage_locations SET name = ?, parent_id = ? WHERE id = ?")
                    .bind_refs([&name, &parent_id, &id])
                    .map_err(Self::map_error)?
                    .run()
                    .await
                    .map_err(Self::map_error)?
                    .meta()
                    .map_err(Self::map_error)?
                    .and_then(|meta| meta.changes)
                    .unwrap_or_default()
            }
            (Some(name), None) => {
                let name = D1Type::Text(name);
                self.db
                    .prepare("UPDATE storage_locations SET name = ? WHERE id = ?")
                    .bind_refs([&name, &id])
                    .map_err(Self::map_error)?
                    .run()
                    .await
                    .map_err(Self::map_error)?
                    .meta()
                    .map_err(Self::map_error)?
                    .and_then(|meta| meta.changes)
                    .unwrap_or_default()
            }
            (None, Some(parent_id)) => {
                let parent_id = match parent_id {
                    Some(parent_id) => Self::bind_id(parent_id)?,
                    None => D1Type::Null,
                };
                self.db
                    .prepare("UPDATE storage_locations SET parent_id = ? WHERE id = ?")
                    .bind_refs([&parent_id, &id])
                    .map_err(Self::map_error)?
                    .run()
                    .await
                    .map_err(Self::map_error)?
                    .meta()
                    .map_err(Self::map_error)?
                    .and_then(|meta| meta.changes)
                    .unwrap_or_default()
            }
            (None, None) => return Ok(()),
        };
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        let id = Self::bind_id(id)?;
        let result = self
            .db
            .prepare("DELETE FROM storage_locations WHERE id = ?")
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
