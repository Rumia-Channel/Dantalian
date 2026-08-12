use super::error::AppError;
use crate::{
    domain::storage_location::StorageLocation,
    ports::storage_location_repository::StorageLocationRepository,
};

pub struct StorageLocationService<R> {
    repo: R,
}

impl<R> StorageLocationService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: StorageLocationRepository> StorageLocationService<R> {
    pub async fn list(&self) -> Result<Vec<StorageLocation>, AppError> {
        self.repo.list().await
    }

    pub async fn create(
        &self,
        name: &str,
        parent_id: Option<i64>,
    ) -> Result<StorageLocation, AppError> {
        let name = normalize_name(name)?;
        self.repo.create(&name, parent_id).await
    }

    pub async fn update(
        &self,
        id: i64,
        name: Option<&str>,
        parent_id: Option<Option<i64>>,
    ) -> Result<(), AppError> {
        let name = name.map(normalize_name).transpose()?;
        self.repo.update(id, name.as_deref(), parent_id).await
    }

    pub async fn delete(&self, id: i64) -> Result<(), AppError> {
        self.repo.delete(id).await
    }
}

fn normalize_name(name: &str) -> Result<String, AppError> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::Validation("Name is required".to_string()));
    }
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct FakeRepository {
        locations: Arc<Mutex<Vec<StorageLocation>>>,
        next_id: Arc<Mutex<i64>>,
    }

    impl StorageLocationRepository for FakeRepository {
        async fn list(&self) -> Result<Vec<StorageLocation>, AppError> {
            Ok(self.locations.lock().unwrap().clone())
        }

        async fn create(
            &self,
            name: &str,
            parent_id: Option<i64>,
        ) -> Result<StorageLocation, AppError> {
            let mut next_id = self.next_id.lock().unwrap();
            let location = StorageLocation {
                id: *next_id,
                name: name.to_string(),
                parent_id,
            };
            *next_id += 1;
            self.locations.lock().unwrap().push(location.clone());
            Ok(location)
        }

        async fn update(
            &self,
            id: i64,
            name: Option<&str>,
            parent_id: Option<Option<i64>>,
        ) -> Result<(), AppError> {
            let mut locations = self.locations.lock().unwrap();
            let location = locations
                .iter_mut()
                .find(|location| location.id == id)
                .ok_or(AppError::NotFound)?;
            if let Some(name) = name {
                location.name = name.to_string();
            }
            if let Some(parent_id) = parent_id {
                location.parent_id = parent_id;
            }
            Ok(())
        }

        async fn delete(&self, id: i64) -> Result<(), AppError> {
            let mut locations = self.locations.lock().unwrap();
            let original_len = locations.len();
            locations.retain(|location| location.id != id);
            if locations.len() == original_len {
                return Err(AppError::NotFound);
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn create_normalizes_name_and_preserves_parent() {
        let service = StorageLocationService::new(FakeRepository::default());
        let location = service.create("  Shelf A  ", Some(7)).await.unwrap();
        assert_eq!(location.name, "Shelf A");
        assert_eq!(location.parent_id, Some(7));
    }

    #[tokio::test]
    async fn empty_name_is_rejected() {
        let service = StorageLocationService::new(FakeRepository::default());
        assert_eq!(
            service.create("  ", None).await,
            Err(AppError::Validation("Name is required".to_string()))
        );
    }

    #[tokio::test]
    async fn update_distinguishes_absent_and_explicit_null_parent() {
        let service = StorageLocationService::new(FakeRepository::default());
        let location = service.create("Shelf", Some(7)).await.unwrap();
        service
            .update(location.id, Some("New Shelf"), None)
            .await
            .unwrap();
        assert_eq!(service.list().await.unwrap()[0].parent_id, Some(7));
        service.update(location.id, None, Some(None)).await.unwrap();
        assert_eq!(service.list().await.unwrap()[0].parent_id, None);
    }
}
