use std::future::Future;

use crate::{application::error::AppError, domain::storage_location::StorageLocation};

pub trait StorageLocationRepository {
    fn list(&self) -> impl Future<Output = Result<Vec<StorageLocation>, AppError>>;

    fn create(
        &self,
        name: &str,
        parent_id: Option<i64>,
    ) -> impl Future<Output = Result<StorageLocation, AppError>>;

    fn update(
        &self,
        id: i64,
        name: Option<&str>,
        parent_id: Option<Option<i64>>,
    ) -> impl Future<Output = Result<(), AppError>>;

    fn delete(&self, id: i64) -> impl Future<Output = Result<(), AppError>>;
}
