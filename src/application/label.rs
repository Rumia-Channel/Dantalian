use super::error::AppError;
use crate::{domain::label::Label, ports::label_repository::LabelRepository};

pub struct LabelService<R> {
    repo: R,
}

impl<R> LabelService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: LabelRepository> LabelService<R> {
    pub async fn list(&self) -> Result<Vec<Label>, AppError> {
        self.repo.list().await
    }

    pub async fn create(&self, name: &str) -> Result<Label, AppError> {
        let name = normalize_name(name)?;
        self.repo.get_or_create(&name).await
    }

    pub async fn rename(&self, id: i64, name: &str) -> Result<(), AppError> {
        let name = normalize_name(name)?;
        self.repo.rename(id, &name).await
    }

    pub async fn delete(&self, id: i64) -> Result<(), AppError> {
        self.repo.delete(id).await
    }
}

fn normalize_name(name: &str) -> Result<String, AppError> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::Validation("Label name is required".to_string()));
    }
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct FakeRepository {
        labels: Arc<Mutex<Vec<Label>>>,
        next_id: Arc<Mutex<i64>>,
    }

    impl LabelRepository for FakeRepository {
        async fn list(&self) -> Result<Vec<Label>, AppError> {
            Ok(self.labels.lock().unwrap().clone())
        }

        async fn get_or_create(&self, name: &str) -> Result<Label, AppError> {
            let mut labels = self.labels.lock().unwrap();
            if let Some(label) = labels.iter().find(|label| label.name == name) {
                return Ok(label.clone());
            }
            let mut next_id = self.next_id.lock().unwrap();
            let label = Label {
                id: *next_id,
                name: name.to_string(),
            };
            *next_id += 1;
            labels.push(label.clone());
            Ok(label)
        }

        async fn rename(&self, id: i64, name: &str) -> Result<(), AppError> {
            let mut labels = self.labels.lock().unwrap();
            let label = labels
                .iter_mut()
                .find(|label| label.id == id)
                .ok_or(AppError::NotFound)?;
            label.name = name.to_string();
            Ok(())
        }

        async fn delete(&self, id: i64) -> Result<(), AppError> {
            let mut labels = self.labels.lock().unwrap();
            let original_len = labels.len();
            labels.retain(|label| label.id != id);
            if labels.len() == original_len {
                return Err(AppError::NotFound);
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn create_normalizes_name_and_reuses_existing_label() {
        let service = LabelService::new(FakeRepository::default());
        let first = service.create("  Fiction  ").await.unwrap();
        let second = service.create("Fiction").await.unwrap();
        assert_eq!(first, second);
        assert_eq!(service.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn empty_name_is_rejected() {
        let service = LabelService::new(FakeRepository::default());
        assert_eq!(
            service.create("  ").await,
            Err(AppError::Validation("Label name is required".to_string()))
        );
    }

    #[tokio::test]
    async fn rename_and_delete_propagate_repository_behavior() {
        let service = LabelService::new(FakeRepository::default());
        let label = service.create("Before").await.unwrap();
        service.rename(label.id, "After").await.unwrap();
        assert_eq!(service.list().await.unwrap()[0].name, "After");
        service.delete(label.id).await.unwrap();
        assert!(service.list().await.unwrap().is_empty());
        assert_eq!(service.delete(label.id).await, Err(AppError::NotFound));
    }
}
