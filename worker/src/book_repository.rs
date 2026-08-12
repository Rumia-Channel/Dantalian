use dantalian::{
    application::error::AppError,
    domain::{
        author::Author,
        book::{BookDetail, BookSummary},
    },
    ports::book_repository::BookRepository,
};
use worker::{D1Database, D1Type};

const BOOK_COLUMNS: &str = "id, isbn, isdn, jan, title, publisher, publish_date, cover_url, description, series_id, series_number, media_type";

pub struct D1BookRepository {
    db: D1Database,
}

impl D1BookRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    fn map_error(error: worker::Error) -> AppError {
        AppError::Database(error.to_string())
    }

    fn bind_id(id: i64) -> Result<D1Type<'static>, AppError> {
        let id = i32::try_from(id)
            .map_err(|_| AppError::Validation("Book id is out of range".to_string()))?;
        if id <= 0 {
            return Err(AppError::Validation("Book id must be positive".to_string()));
        }
        Ok(D1Type::Integer(id))
    }
}

impl BookRepository for D1BookRepository {
    async fn list(&self) -> Result<Vec<BookSummary>, AppError> {
        self.db
            .prepare(&format!(
                "SELECT {BOOK_COLUMNS} FROM books ORDER BY id DESC"
            ))
            .all()
            .await
            .map_err(Self::map_error)?
            .results::<BookSummary>()
            .map_err(Self::map_error)
    }

    async fn get(&self, id: i64) -> Result<BookDetail, AppError> {
        let id = Self::bind_id(id)?;
        let book = self
            .db
            .prepare(&format!("SELECT {BOOK_COLUMNS} FROM books WHERE id = ?"))
            .bind_refs(&id)
            .map_err(Self::map_error)?
            .first::<BookSummary>(None)
            .await
            .map_err(Self::map_error)?
            .ok_or(AppError::NotFound)?;

        let authors = self
            .db
            .prepare(
                "SELECT a.id, a.ndl_id, a.name, a.transcription
                 FROM authors a
                 JOIN book_authors ba ON ba.author_id = a.id
                 WHERE ba.book_id = ?
                 ORDER BY ba.sort_order, ba.author_id",
            )
            .bind_refs(&id)
            .map_err(Self::map_error)?
            .all()
            .await
            .map_err(Self::map_error)?
            .results::<Author>()
            .map_err(Self::map_error)?;

        Ok(BookDetail { book, authors })
    }
}
