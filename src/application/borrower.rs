use super::error::AppError;
use crate::{domain::borrower::Borrower, ports::borrower_repository::BorrowerRepository};

pub struct BorrowerService<R> {
    repo: R,
}

impl<R> BorrowerService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: BorrowerRepository> BorrowerService<R> {
    pub async fn list(&self) -> Result<Vec<Borrower>, AppError> {
        self.repo.list().await
    }

    pub async fn create(&self, name: &str, notes: Option<&str>) -> Result<Borrower, AppError> {
        let name = normalize_name(name)?;
        self.repo.create(&name, notes).await
    }

    pub async fn update(
        &self,
        id: i64,
        name: Option<&str>,
        notes: Option<&str>,
    ) -> Result<(), AppError> {
        let name = name.map(normalize_name).transpose()?;
        self.repo.update(id, name.as_deref(), notes).await
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
        borrowers: Arc<Mutex<Vec<Borrower>>>,
        next_id: Arc<Mutex<i64>>,
    }

    impl BorrowerRepository for FakeRepository {
        async fn list(&self) -> Result<Vec<Borrower>, AppError> {
            Ok(self.borrowers.lock().unwrap().clone())
        }

        async fn create(&self, name: &str, notes: Option<&str>) -> Result<Borrower, AppError> {
            let mut next_id = self.next_id.lock().unwrap();
            let borrower = Borrower {
                id: *next_id,
                name: name.to_string(),
                notes: notes.map(str::to_string),
            };
            *next_id += 1;
            self.borrowers.lock().unwrap().push(borrower.clone());
            Ok(borrower)
        }

        async fn update(
            &self,
            id: i64,
            name: Option<&str>,
            notes: Option<&str>,
        ) -> Result<(), AppError> {
            let mut borrowers = self.borrowers.lock().unwrap();
            let borrower = borrowers
                .iter_mut()
                .find(|borrower| borrower.id == id)
                .ok_or(AppError::NotFound)?;
            if let Some(name) = name {
                borrower.name = name.to_string();
            }
            borrower.notes = notes.map(str::to_string);
            Ok(())
        }

        async fn delete(&self, id: i64) -> Result<(), AppError> {
            let mut borrowers = self.borrowers.lock().unwrap();
            let original_len = borrowers.len();
            borrowers.retain(|borrower| borrower.id != id);
            if borrowers.len() == original_len {
                return Err(AppError::NotFound);
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn create_normalizes_name() {
        let service = BorrowerService::new(FakeRepository::default());
        let borrower = service.create("  Alice  ", Some("note")).await.unwrap();
        assert_eq!(borrower.name, "Alice");
        assert_eq!(borrower.notes.as_deref(), Some("note"));
    }

    #[tokio::test]
    async fn empty_name_is_rejected() {
        let service = BorrowerService::new(FakeRepository::default());
        assert_eq!(
            service.create("  ", None).await,
            Err(AppError::Validation("Name is required".to_string()))
        );
    }

    #[tokio::test]
    async fn update_and_delete_propagate_repository_behavior() {
        let service = BorrowerService::new(FakeRepository::default());
        let borrower = service.create("Before", None).await.unwrap();
        service
            .update(borrower.id, Some("After"), Some("updated"))
            .await
            .unwrap();
        let updated = &service.list().await.unwrap()[0];
        assert_eq!(updated.name, "After");
        assert_eq!(updated.notes.as_deref(), Some("updated"));
        service.delete(borrower.id).await.unwrap();
        assert!(service.list().await.unwrap().is_empty());
    }
}
