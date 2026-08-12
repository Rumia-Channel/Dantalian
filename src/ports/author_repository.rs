use std::future::Future;

use crate::{application::error::AppError, domain::author::Author};

pub trait AuthorRepository {
    fn list(&self) -> impl Future<Output = Result<Vec<Author>, AppError>>;

    fn get(&self, id: i64) -> impl Future<Output = Result<Author, AppError>>;

    fn create(
        &self,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> impl Future<Output = Result<Author, AppError>>;

    fn update(
        &self,
        id: i64,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> impl Future<Output = Result<(), AppError>>;

    fn delete(&self, id: i64) -> impl Future<Output = Result<(), AppError>>;
}
