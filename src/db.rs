use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Author {
    pub id: i64,
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewAuthor {
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Book {
    pub id: i64,
    pub isbn: String,
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub alternative: Option<String>,
    pub alternative_transcription: Option<String>,
    pub volume: Option<String>,
    pub volume_transcription: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub jpno: Option<String>,
    pub ndl_url: Option<String>,
    pub series_id: Option<i64>,
    pub series_number: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BookWithAuthors {
    #[serde(flatten)]
    pub book: Book,
    pub authors: Vec<Author>,
}

#[derive(Debug, Deserialize)]
pub struct NewBook {
    pub isbn: String,
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub alternative: Option<String>,
    pub alternative_transcription: Option<String>,
    pub volume: Option<String>,
    pub volume_transcription: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub jpno: Option<String>,
    pub ndl_url: Option<String>,
    pub authors: Vec<NewAuthor>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Series {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrandSeries {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrandSeriesItemInfo {
    pub item_type: String,
    pub item_id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrandSeriesWithItems {
    pub id: i64,
    pub name: String,
    pub items: Vec<GrandSeriesItemInfo>,
}

#[derive(Clone)]
pub struct Db(pub Arc<Mutex<Connection>>);

impl Db {
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS series (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS grand_series (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS grand_series_items (
                grand_series_id INTEGER NOT NULL REFERENCES grand_series(id) ON DELETE CASCADE,
                item_type TEXT NOT NULL CHECK(item_type IN ('series', 'book')),
                item_id INTEGER NOT NULL,
                PRIMARY KEY (grand_series_id, item_type, item_id)
            );
            CREATE TABLE IF NOT EXISTS authors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ndl_id TEXT UNIQUE,
                name TEXT NOT NULL,
                transcription TEXT
            );
            CREATE TABLE IF NOT EXISTS books (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                isbn TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                publisher TEXT,
                publish_date TEXT,
                cover_url TEXT,
                description TEXT,
                title_transcription TEXT,
                series_title TEXT,
                series_title_transcription TEXT,
                alternative TEXT,
                alternative_transcription TEXT,
                volume TEXT,
                volume_transcription TEXT,
                price TEXT,
                extent TEXT,
                jpno TEXT,
                ndl_url TEXT,
                series_id INTEGER REFERENCES series(id) ON DELETE SET NULL,
                series_number INTEGER
            );
            CREATE TABLE IF NOT EXISTS book_authors (
                book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
                author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
                sort_order INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (book_id, author_id)
            );",
        )?;
        conn.execute_batch(
            "ALTER TABLE book_authors ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;"
        ).ok();
        conn.execute_batch(
            "ALTER TABLE books ADD COLUMN series_number INTEGER;"
        ).ok();
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    fn row_to_book(row: &Row<'_>) -> rusqlite::Result<Book> {
        Ok(Book {
            id: row.get(0)?,
            isbn: row.get(1)?,
            title: row.get(2)?,
            publisher: row.get(3)?,
            publish_date: row.get(4)?,
            cover_url: row.get(5)?,
            description: row.get(6)?,
            title_transcription: row.get(7)?,
            series_title: row.get(8)?,
            series_title_transcription: row.get(9)?,
            alternative: row.get(10)?,
            alternative_transcription: row.get(11)?,
            volume: row.get(12)?,
            volume_transcription: row.get(13)?,
            price: row.get(14)?,
            extent: row.get(15)?,
            jpno: row.get(16)?,
            ndl_url: row.get(17)?,
            series_id: row.get(18)?,
            series_number: row.get(19)?,
        })
    }

    fn row_to_author(row: &Row<'_>) -> rusqlite::Result<Author> {
        Ok(Author {
            id: row.get(0)?,
            ndl_id: row.get(1)?,
            name: row.get(2)?,
            transcription: row.get(3)?,
        })
    }

    pub fn insert_book(&self, book: &NewBook) -> Result<Book, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO books (isbn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![book.isbn, book.title, book.publisher, book.publish_date, book.cover_url, book.description, book.title_transcription, book.series_title, book.series_title_transcription, book.alternative, book.alternative_transcription, book.volume, book.volume_transcription, book.price, book.extent, book.jpno, book.ndl_url],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Book {
            id,
            isbn: book.isbn.clone(),
            title: book.title.clone(),
            publisher: book.publisher.clone(),
            publish_date: book.publish_date.clone(),
            cover_url: book.cover_url.clone(),
            description: book.description.clone(),
            title_transcription: book.title_transcription.clone(),
            series_title: book.series_title.clone(),
            series_title_transcription: book.series_title_transcription.clone(),
            alternative: book.alternative.clone(),
            alternative_transcription: book.alternative_transcription.clone(),
            volume: book.volume.clone(),
            volume_transcription: book.volume_transcription.clone(),
            price: book.price.clone(),
            extent: book.extent.clone(),
            jpno: book.jpno.clone(),
            ndl_url: book.ndl_url.clone(),
            series_id: None,
                series_number: None,
        })
    }

    pub fn insert_author(
        &self,
        ndl_id: Option<&str>,
        name: &str,
        transcription: Option<&str>,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.0.lock().unwrap();

        if let Some(nid) = ndl_id {
            let mut stmt = conn.prepare("SELECT id FROM authors WHERE ndl_id = ?1")?;
            if let Some(row) = stmt
                .query_row(params![nid], |row| row.get::<_, i64>(0))
                .ok()
            {
                return Ok(row);
            }
        }

        conn.execute(
            "INSERT INTO authors (ndl_id, name, transcription) VALUES (?1, ?2, ?3)",
            params![ndl_id, name, transcription],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn add_book_author(&self, book_id: i64, author_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO book_authors (book_id, author_id) VALUES (?1, ?2)",
            params![book_id, author_id],
        )?;
        Ok(())
    }

    pub fn get_book_authors(&self, book_id: i64) -> Result<Vec<Author>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.ndl_id, a.name, a.transcription
             FROM authors a
             JOIN book_authors ba ON a.id = ba.author_id
             WHERE ba.book_id = ?1
             ORDER BY ba.sort_order, ba.author_id",
        )?;
        let rows = stmt.query_map(params![book_id], Self::row_to_author)?;
        rows.collect()
    }

    pub fn get_author_by_id(&self, id: i64) -> Result<Option<Author>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, ndl_id, name, transcription FROM authors WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], Self::row_to_author)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn get_author_by_ndl_id(&self, ndl_id: &str) -> Result<Option<Author>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, ndl_id, name, transcription FROM authors WHERE ndl_id = ?1")?;
        let mut rows = stmt.query_map(params![ndl_id], Self::row_to_author)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_books(&self) -> Result<Vec<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number FROM books ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_book)?;
        rows.collect()
    }

    pub fn find_by_isbn(&self, isbn: &str) -> Result<Option<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number FROM books WHERE isbn = ?1",
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

    pub fn update_book(
        &self,
        id: i64,
        title: &str,
        publisher: Option<&str>,
        publish_date: Option<&str>,
        description: Option<&str>,
        title_transcription: Option<&str>,
        series_title: Option<&str>,
        series_title_transcription: Option<&str>,
        alternative: Option<&str>,
        alternative_transcription: Option<&str>,
        volume: Option<&str>,
        volume_transcription: Option<&str>,
        price: Option<&str>,
        extent: Option<&str>,
        jpno: Option<&str>,
        ndl_url: Option<&str>,
        series_id: Option<i64>,
        series_number: Option<i64>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE books SET title=?1, publisher=?2, publish_date=?3, description=?4,
             title_transcription=?5, series_title=?6, series_title_transcription=?7,
             alternative=?8, alternative_transcription=?9, volume=?10, volume_transcription=?11,
             price=?12, extent=?13, jpno=?14, ndl_url=?15,
             series_id=?16, series_number=?17
             WHERE id=?18",
            params![
                title, publisher, publish_date, description,
                title_transcription, series_title, series_title_transcription,
                alternative, alternative_transcription, volume, volume_transcription,
                price, extent, jpno, ndl_url,
                series_id, series_number, id,
            ],
        )?;
        Ok(affected > 0)
    }

    pub fn update_author(
        &self,
        id: i64,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE authors SET name=?1, transcription=?2, ndl_id=?3 WHERE id=?4",
            params![name, transcription, ndl_id, id],
        )?;
        Ok(affected > 0)
    }

    pub fn create_author(
        &self,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> Result<Author, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO authors (ndl_id, name, transcription) VALUES (?1, ?2, ?3)",
            params![ndl_id, name, transcription],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Author {
            id,
            ndl_id: ndl_id.map(|s| s.to_string()),
            name: name.to_string(),
            transcription: transcription.map(|s| s.to_string()),
        })
    }

    pub fn list_authors(&self) -> Result<Vec<Author>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, ndl_id, name, transcription FROM authors ORDER BY id")?;
        let rows = stmt.query_map([], Self::row_to_author)?;
        rows.collect()
    }

    pub fn remove_book_author(
        &self,
        book_id: i64,
        author_id: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM book_authors WHERE book_id = ?1 AND author_id = ?2",
            params![book_id, author_id],
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

    pub fn create_grand_series(&self, name: &str) -> Result<GrandSeries, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("INSERT INTO grand_series (name) VALUES (?1)", params![name])?;
        let id = conn.last_insert_rowid();
        Ok(GrandSeries {
            id,
            name: name.to_string(),
        })
    }

    pub fn list_grand_series(&self) -> Result<Vec<GrandSeriesWithItems>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT gs.id, gs.name, gsi.item_type, gsi.item_id,
                    COALESCE(s.name, b.title) AS item_name
             FROM grand_series gs
             LEFT JOIN grand_series_items gsi ON gs.id = gsi.grand_series_id
             LEFT JOIN series s ON gsi.item_type = 'series' AND s.id = gsi.item_id
             LEFT JOIN books b ON gsi.item_type = 'book' AND b.id = gsi.item_id
             ORDER BY gs.id, gsi.rowid",
        )?;
        let mut map: std::collections::HashMap<i64, GrandSeriesWithItems> =
            std::collections::HashMap::new();
        let rows = stmt.query_map([], |row| {
            let gs_id: i64 = row.get(0)?;
            let gs_name: String = row.get(1)?;
            let item_type: Option<String> = row.get(2)?;
            let item_id: Option<i64> = row.get(3)?;
            let item_name: Option<String> = row.get(4)?;
            Ok((gs_id, gs_name, item_type, item_id, item_name))
        })?;
        for row in rows {
            let (gs_id, gs_name, item_type, item_id, item_name) = row?;
            let entry = map.entry(gs_id).or_insert_with(|| GrandSeriesWithItems {
                id: gs_id,
                name: gs_name,
                items: Vec::new(),
            });
            if let (Some(it), Some(iid), Some(iname)) = (item_type, item_id, item_name) {
                entry.items.push(GrandSeriesItemInfo {
                    item_type: it,
                    item_id: iid,
                    name: iname,
                });
            }
        }
        Ok(map.into_values().collect())
    }

    pub fn rename_grand_series(&self, id: i64, name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE grand_series SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_grand_series(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM grand_series WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn add_grand_series_item(
        &self,
        grand_series_id: i64,
        item_type: &str,
        item_id: i64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO grand_series_items (grand_series_id, item_type, item_id) VALUES (?1, ?2, ?3)",
            params![grand_series_id, item_type, item_id],
        )?;
        Ok(())
    }

    pub fn remove_grand_series_item(
        &self,
        grand_series_id: i64,
        item_type: &str,
        item_id: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM grand_series_items WHERE grand_series_id = ?1 AND item_type = ?2 AND item_id = ?3",
            params![grand_series_id, item_type, item_id],
        )?;
        Ok(affected > 0)
    }

    pub fn get_book_grand_series(
        &self,
        book_id: i64,
    ) -> Result<Option<GrandSeries>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT gs.id, gs.name FROM grand_series gs
             JOIN grand_series_items gsi ON gs.id = gsi.grand_series_id
             WHERE gsi.item_type = 'book' AND gsi.item_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![book_id], |row| {
            Ok(GrandSeries {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn get_series_grand_series(
        &self,
        series_id: i64,
    ) -> Result<Option<GrandSeries>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT gs.id, gs.name FROM grand_series gs
             JOIN grand_series_items gsi ON gs.id = gsi.grand_series_id
             WHERE gsi.item_type = 'series' AND gsi.item_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![series_id], |row| {
            Ok(GrandSeries {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }
}
