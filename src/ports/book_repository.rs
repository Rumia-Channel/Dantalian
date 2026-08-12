use std::future::Future;

use crate::{
    application::error::AppError,
    domain::book::{BookDetail, BookSummary},
};

pub trait BookRepository {
    fn list(&self) -> impl Future<Output = Result<Vec<BookSummary>, AppError>>;
    fn get(&self, id: i64) -> impl Future<Output = Result<BookDetail, AppError>>;
}
