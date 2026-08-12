use std::future::Future;

use crate::{application::error::AppError, domain::series::Series};

pub trait SeriesRepository {
    fn list(&self) -> impl Future<Output = Result<Vec<Series>, AppError>>;

    fn create(&self, name: &str) -> impl Future<Output = Result<Series, AppError>>;

    fn rename(&self, id: i64, name: &str) -> impl Future<Output = Result<(), AppError>>;

    fn delete(&self, id: i64) -> impl Future<Output = Result<(), AppError>>;
}
