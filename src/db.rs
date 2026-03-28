use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Book {
    pub id: i64,
    pub isbn: String,
    pub title: String,
    pub author: Option<String>,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub creator_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub edition: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub subject: Option<String>,
    pub ndl_url: Option<String>,
    pub series_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Series {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct NewBook {
    pub isbn: String,
    pub title: String,
    pub author: Option<String>,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub creator_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub edition: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub subject: Option<String>,
    pub ndl_url: Option<String>,
}

#[derive(Clone)]
pub struct Db(pub Arc<Mutex<Connection>>);

impl Db {
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("DROP TABLE IF EXISTS books; DROP TABLE IF EXISTS series;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS series (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS books (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                isbn TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                author TEXT,
                publisher TEXT,
                publish_date TEXT,
                cover_url TEXT,
                description TEXT,
                title_transcription TEXT,
                creator_transcription TEXT,
                series_title TEXT,
                series_title_transcription TEXT,
                edition TEXT,
                price TEXT,
                extent TEXT,
                subject TEXT,
                ndl_url TEXT,
                series_id INTEGER REFERENCES series(id) ON DELETE SET NULL
            );",
        )?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    fn row_to_book(row: &Row<'_>) -> rusqlite::Result<Book> {
        Ok(Book {
            id: row.get(0)?,
            isbn: row.get(1)?,
            title: row.get(2)?,
            author: row.get(3)?,
            publisher: row.get(4)?,
            publish_date: row.get(5)?,
            cover_url: row.get(6)?,
            description: row.get(7)?,
            title_transcription: row.get(8)?,
            creator_transcription: row.get(9)?,
            series_title: row.get(10)?,
            series_title_transcription: row.get(11)?,
            edition: row.get(12)?,
            price: row.get(13)?,
            extent: row.get(14)?,
            subject: row.get(15)?,
            ndl_url: row.get(16)?,
            series_id: row.get(17)?,
        })
    }

    pub fn insert_book(&self, book: &NewBook) -> Result<Book, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO books (isbn, title, author, publisher, publish_date, cover_url, description, title_transcription, creator_transcription, series_title, series_title_transcription, edition, price, extent, subject, ndl_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![book.isbn, book.title, book.author, book.publisher, book.publish_date, book.cover_url, book.description, book.title_transcription, book.creator_transcription, book.series_title, book.series_title_transcription, book.edition, book.price, book.extent, book.subject, book.ndl_url],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Book {
            id,
            isbn: book.isbn.clone(),
            title: book.title.clone(),
            author: book.author.clone(),
            publisher: book.publisher.clone(),
            publish_date: book.publish_date.clone(),
            cover_url: book.cover_url.clone(),
            description: book.description.clone(),
            title_transcription: book.title_transcription.clone(),
            creator_transcription: book.creator_transcription.clone(),
            series_title: book.series_title.clone(),
            series_title_transcription: book.series_title_transcription.clone(),
            edition: book.edition.clone(),
            price: book.price.clone(),
            extent: book.extent.clone(),
            subject: book.subject.clone(),
            ndl_url: book.ndl_url.clone(),
            series_id: None,
        })
    }

    pub fn list_books(&self) -> Result<Vec<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, title, author, publisher, publish_date, cover_url, description, title_transcription, creator_transcription, series_title, series_title_transcription, edition, price, extent, subject, ndl_url, series_id FROM books ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_book)?;
        rows.collect()
    }

    pub fn find_by_isbn(&self, isbn: &str) -> Result<Option<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, title, author, publisher, publish_date, cover_url, description, title_transcription, creator_transcription, series_title, series_title_transcription, edition, price, extent, subject, ndl_url, series_id FROM books WHERE isbn = ?1",
        )?;
        let mut rows = stmt.query_map(params![isbn], Self::row_to_book)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn delete_book(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM books WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn set_book_series(
        &self,
        book_id: i64,
        series_id: Option<i64>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE books SET series_id = ?1 WHERE id = ?2",
            params![series_id, book_id],
        )?;
        Ok(affected > 0)
    }

    pub fn create_series(&self, name: &str) -> Result<Series, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("INSERT INTO series (name) VALUES (?1)", params![name])?;
        let id = conn.last_insert_rowid();
        Ok(Series {
            id,
            name: name.to_string(),
        })
    }

    pub fn list_series(&self) -> Result<Vec<Series>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name FROM series ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(Series {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn rename_series(&self, id: i64, name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE series SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_series(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM series WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }
}
