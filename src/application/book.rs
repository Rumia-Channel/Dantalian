use crate::{
    domain::book::{BookDetail, BookSummary},
    ports::book_repository::BookRepository,
};

use super::error::AppError;

pub struct BookService<R> {
    repo: R,
}

impl<R> BookService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: BookRepository> BookService<R> {
    pub async fn list(&self) -> Result<Vec<BookSummary>, AppError> {
        self.repo.list().await
    }

    pub async fn get(&self, id: i64) -> Result<BookDetail, AppError> {
        self.repo.get(id).await
    }
}
