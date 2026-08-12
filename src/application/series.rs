use super::error::AppError;
use crate::{domain::series::Series, ports::series_repository::SeriesRepository};

pub struct SeriesService<R> {
    repo: R,
}

impl<R> SeriesService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: SeriesRepository> SeriesService<R> {
    pub async fn list(&self) -> Result<Vec<Series>, AppError> {
        self.repo.list().await
    }

    pub async fn create(&self, name: &str) -> Result<Series, AppError> {
        let name = normalize_name(name)?;
        self.repo.create(&name).await
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
        return Err(AppError::Validation("Series name is required".to_string()));
    }
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct FakeRepository {
        series: Arc<Mutex<Vec<Series>>>,
        next_id: Arc<Mutex<i64>>,
    }

    impl SeriesRepository for FakeRepository {
        async fn list(&self) -> Result<Vec<Series>, AppError> {
            Ok(self.series.lock().unwrap().clone())
        }

        async fn create(&self, name: &str) -> Result<Series, AppError> {
            let mut next_id = self.next_id.lock().unwrap();
            let series = Series {
                id: *next_id,
                name: name.to_string(),
            };
            *next_id += 1;
            self.series.lock().unwrap().push(series.clone());
            Ok(series)
        }

        async fn rename(&self, id: i64, name: &str) -> Result<(), AppError> {
            let mut series = self.series.lock().unwrap();
            let item = series
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or(AppError::NotFound)?;
            item.name = name.to_string();
            Ok(())
        }

        async fn delete(&self, id: i64) -> Result<(), AppError> {
            let mut series = self.series.lock().unwrap();
            let original_len = series.len();
            series.retain(|item| item.id != id);
            if series.len() == original_len {
                return Err(AppError::NotFound);
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn create_normalizes_name() {
        let service = SeriesService::new(FakeRepository::default());
        let series = service.create("  Test Series  ").await.unwrap();
        assert_eq!(series.name, "Test Series");
    }

    #[tokio::test]
    async fn empty_name_is_rejected() {
        let service = SeriesService::new(FakeRepository::default());
        assert_eq!(
            service.create("  ").await,
            Err(AppError::Validation("Series name is required".to_string()))
        );
    }

    #[tokio::test]
    async fn rename_and_delete_propagate_repository_behavior() {
        let service = SeriesService::new(FakeRepository::default());
        let series = service.create("Before").await.unwrap();
        service.rename(series.id, "After").await.unwrap();
        assert_eq!(service.list().await.unwrap()[0].name, "After");
        service.delete(series.id).await.unwrap();
        assert!(service.list().await.unwrap().is_empty());
        assert_eq!(service.delete(series.id).await, Err(AppError::NotFound));
    }
}
