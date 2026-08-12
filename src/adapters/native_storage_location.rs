use crate::{
    application::error::AppError, db::Db, domain::storage_location::StorageLocation,
    ports::storage_location_repository::StorageLocationRepository,
};

#[derive(Clone)]
pub struct NativeStorageLocationRepository {
    db: Db,
}

impl NativeStorageLocationRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl StorageLocationRepository for NativeStorageLocationRepository {
    async fn list(&self) -> Result<Vec<StorageLocation>, AppError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.list_storage_locations()
                .map(|locations| locations.into_iter().map(Into::into).collect())
                .map_err(|error| AppError::Database(error.to_string()))
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn create(
        &self,
        name: &str,
        parent_id: Option<i64>,
    ) -> Result<StorageLocation, AppError> {
        let db = self.db.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            db.create_storage_location(&name, parent_id)
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
        parent_id: Option<Option<i64>>,
    ) -> Result<(), AppError> {
        let db = self.db.clone();
        let name = name.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            if let Some(name) = name.as_deref() {
                let updated = db
                    .rename_storage_location(id, name)
                    .map_err(|error| AppError::Database(error.to_string()))?;
                if !updated {
                    return Err(AppError::NotFound);
                }
            }
            if let Some(parent_id) = parent_id {
                let updated = db
                    .set_storage_location_parent(id, parent_id)
                    .map_err(|error| AppError::Database(error.to_string()))?;
                if !updated {
                    return Err(AppError::NotFound);
                }
            }
            if name.is_none() && parent_id.is_none() {
                let exists = db
                    .get_storage_location(id)
                    .map_err(|error| AppError::Database(error.to_string()))?
                    .is_some();
                if !exists {
                    return Err(AppError::NotFound);
                }
            }
            Ok(())
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        let db = self.db.clone();
        let deleted = tokio::task::spawn_blocking(move || db.delete_storage_location(id))
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

impl From<crate::db::StorageLocation> for StorageLocation {
    fn from(location: crate::db::StorageLocation) -> Self {
        Self {
            id: location.id,
            name: location.name,
            parent_id: location.parent_id,
        }
    }
}
