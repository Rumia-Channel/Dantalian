use std::future::Future;

use crate::{application::error::AppError, domain::borrower::Borrower};

pub trait BorrowerRepository {
    fn list(&self) -> impl Future<Output = Result<Vec<Borrower>, AppError>>;

    fn create(
        &self,
        name: &str,
        notes: Option<&str>,
    ) -> impl Future<Output = Result<Borrower, AppError>>;

    fn update(
        &self,
        id: i64,
        name: Option<&str>,
        notes: Option<&str>,
    ) -> impl Future<Output = Result<(), AppError>>;

    fn delete(&self, id: i64) -> impl Future<Output = Result<(), AppError>>;
}
