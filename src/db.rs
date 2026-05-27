pub use crate::db_models::*;
use rusqlite::{Connection, Row, params};
use std::sync::{Arc, Mutex};

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
                isbn TEXT UNIQUE,
                isdn TEXT UNIQUE,
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
                series_number INTEGER,
                isdn_region TEXT,
                isdn_class TEXT,
                isdn_type TEXT,
                isdn_rating_gender TEXT,
                isdn_rating_age TEXT,
                isdn_genre_code TEXT,
                isdn_genre_name TEXT,
                isdn_genre_user TEXT,
                isdn_c_code TEXT,
                isdn_author TEXT,
                isdn_shape TEXT,
                isdn_contents TEXT,
                isdn_barcode2 TEXT,
                isdn_sample_image_url TEXT,
                isdn_useroption TEXT,
                isdn_external_links TEXT
            );
            CREATE TABLE IF NOT EXISTS book_authors (
                book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
                author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
                sort_order INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (book_id, author_id)
            );
            CREATE TABLE IF NOT EXISTS copies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
                copy_type TEXT NOT NULL DEFAULT 'physical' CHECK(copy_type IN ('physical', 'ebook')),
                location TEXT,
                notes TEXT
            );
            CREATE TABLE IF NOT EXISTS borrowers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                notes TEXT
            );
            CREATE TABLE IF NOT EXISTS lending_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                copy_id INTEGER NOT NULL REFERENCES copies(id) ON DELETE CASCADE,
                borrower_id INTEGER NOT NULL REFERENCES borrowers(id),
                lent_date TEXT NOT NULL,
                due_date TEXT,
                returned_date TEXT,
                notes TEXT
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        conn.execute_batch(
            "ALTER TABLE book_authors ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;",
        )
        .ok();
        conn.execute_batch("ALTER TABLE books ADD COLUMN series_number INTEGER;")
            .ok();
        conn.execute_batch("ALTER TABLE books ADD COLUMN isdn TEXT;")
            .ok();
        conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_books_isdn ON books(isdn) WHERE isdn IS NOT NULL;")
            .ok();
        let isdn_cols = [
            "isdn_region TEXT",
            "isdn_class TEXT",
            "isdn_type TEXT",
            "isdn_rating_gender TEXT",
            "isdn_rating_age TEXT",
            "isdn_genre_code TEXT",
            "isdn_genre_name TEXT",
            "isdn_genre_user TEXT",
            "isdn_c_code TEXT",
            "isdn_author TEXT",
            "isdn_shape TEXT",
            "isdn_contents TEXT",
            "isdn_barcode2 TEXT",
            "isdn_sample_image_url TEXT",
            "isdn_useroption TEXT",
            "isdn_external_links TEXT",
        ];
        for col in &isdn_cols {
            conn.execute_batch(&format!("ALTER TABLE books ADD COLUMN {};", col))
                .ok();
        }
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    fn row_to_book(row: &Row<'_>) -> rusqlite::Result<Book> {
        Ok(Book {
            id: row.get(0)?,
            isbn: row.get(1)?,
            isdn: row.get(2)?,
            title: row.get(3)?,
            publisher: row.get(4)?,
            publish_date: row.get(5)?,
            cover_url: row.get(6)?,
            description: row.get(7)?,
            title_transcription: row.get(8)?,
            series_title: row.get(9)?,
            series_title_transcription: row.get(10)?,
            alternative: row.get(11)?,
            alternative_transcription: row.get(12)?,
            volume: row.get(13)?,
            volume_transcription: row.get(14)?,
            price: row.get(15)?,
            extent: row.get(16)?,
            jpno: row.get(17)?,
            ndl_url: row.get(18)?,
            series_id: row.get(19)?,
            series_number: row.get(20)?,
            isdn_region: row.get(21)?,
            isdn_class: row.get(22)?,
            isdn_type: row.get(23)?,
            isdn_rating_gender: row.get(24)?,
            isdn_rating_age: row.get(25)?,
            isdn_genre_code: row.get(26)?,
            isdn_genre_name: row.get(27)?,
            isdn_genre_user: row.get(28)?,
            isdn_c_code: row.get(29)?,
            isdn_author: row.get(30)?,
            isdn_shape: row.get(31)?,
            isdn_contents: row.get(32)?,
            isdn_barcode2: row.get(33)?,
            isdn_sample_image_url: row.get(34)?,
            isdn_useroption: row.get(35)?,
            isdn_external_links: row.get(36)?,
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
            "INSERT OR IGNORE INTO books (isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34)",
            params![book.isbn, book.isdn, book.title, book.publisher, book.publish_date, book.cover_url, book.description, book.title_transcription, book.series_title, book.series_title_transcription, book.alternative, book.alternative_transcription, book.volume, book.volume_transcription, book.price, book.extent, book.jpno, book.ndl_url, book.isdn_region, book.isdn_class, book.isdn_type, book.isdn_rating_gender, book.isdn_rating_age, book.isdn_genre_code, book.isdn_genre_name, book.isdn_genre_user, book.isdn_c_code, book.isdn_author, book.isdn_shape, book.isdn_contents, book.isdn_barcode2, book.isdn_sample_image_url, book.isdn_useroption, book.isdn_external_links],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Book {
            id,
            isbn: book.isbn.clone(),
            isdn: book.isdn.clone(),
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
            isdn_region: book.isdn_region.clone(),
            isdn_class: book.isdn_class.clone(),
            isdn_type: book.isdn_type.clone(),
            isdn_rating_gender: book.isdn_rating_gender.clone(),
            isdn_rating_age: book.isdn_rating_age.clone(),
            isdn_genre_code: book.isdn_genre_code.clone(),
            isdn_genre_name: book.isdn_genre_name.clone(),
            isdn_genre_user: book.isdn_genre_user.clone(),
            isdn_c_code: book.isdn_c_code.clone(),
            isdn_author: book.isdn_author.clone(),
            isdn_shape: book.isdn_shape.clone(),
            isdn_contents: book.isdn_contents.clone(),
            isdn_barcode2: book.isdn_barcode2.clone(),
            isdn_sample_image_url: book.isdn_sample_image_url.clone(),
            isdn_useroption: book.isdn_useroption.clone(),
            isdn_external_links: book.isdn_external_links.clone(),
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

    pub fn get_book_authors(&self, book_id: i64) -> Result<Vec<BookAuthor>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.ndl_id, a.name, a.transcription, ba.sort_order
             FROM authors a
             JOIN book_authors ba ON a.id = ba.author_id
             WHERE ba.book_id = ?1
             ORDER BY ba.sort_order, ba.author_id",
        )?;
        let rows = stmt.query_map(params![book_id], |row| {
            Ok(BookAuthor {
                id: row.get(0)?,
                ndl_id: row.get(1)?,
                name: row.get(2)?,
                transcription: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_book_author_order(
        &self,
        book_id: i64,
        author_id: i64,
        sort_order: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE book_authors SET sort_order = ?1 WHERE book_id = ?2 AND author_id = ?3",
            params![sort_order, book_id, author_id],
        )?;
        Ok(affected > 0)
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
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links FROM books ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_book)?;
        rows.collect()
    }

    pub fn find_by_isbn(&self, isbn: &str) -> Result<Option<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links FROM books WHERE isbn = ?1",
        )?;
        let mut rows = stmt.query_map(params![isbn], Self::row_to_book)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn find_by_isdn(&self, isdn: &str) -> Result<Option<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links FROM books WHERE isdn = ?1",
        )?;
        let mut rows = stmt.query_map(params![isdn], Self::row_to_book)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn find_by_id(&self, id: i64) -> Result<Option<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links FROM books WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::row_to_book)?;
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
        isbn: Option<&str>,
        isdn: Option<&str>,
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
        isdn_region: Option<&str>,
        isdn_class: Option<&str>,
        isdn_type: Option<&str>,
        isdn_rating_gender: Option<&str>,
        isdn_rating_age: Option<&str>,
        isdn_genre_code: Option<&str>,
        isdn_genre_name: Option<&str>,
        isdn_genre_user: Option<&str>,
        isdn_c_code: Option<&str>,
        isdn_author: Option<&str>,
        isdn_shape: Option<&str>,
        isdn_contents: Option<&str>,
        isdn_barcode2: Option<&str>,
        isdn_sample_image_url: Option<&str>,
        isdn_useroption: Option<&str>,
        isdn_external_links: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE books SET isbn=?1, isdn=?2, title=?3, publisher=?4, publish_date=?5, description=?6,
             title_transcription=?7, series_title=?8, series_title_transcription=?9,
             alternative=?10, alternative_transcription=?11, volume=?12, volume_transcription=?13,
             price=?14, extent=?15, jpno=?16, ndl_url=?17,
             series_id=?18, series_number=?19,
             isdn_region=?20, isdn_class=?21, isdn_type=?22, isdn_rating_gender=?23, isdn_rating_age=?24,
             isdn_genre_code=?25, isdn_genre_name=?26, isdn_genre_user=?27, isdn_c_code=?28,
             isdn_author=?29, isdn_shape=?30, isdn_contents=?31, isdn_barcode2=?32,
             isdn_sample_image_url=?33, isdn_useroption=?34, isdn_external_links=?35
             WHERE id=?36",
            params![
                isbn, isdn, title, publisher, publish_date, description,
                title_transcription, series_title, series_title_transcription,
                alternative, alternative_transcription, volume, volume_transcription,
                price, extent, jpno, ndl_url,
                series_id, series_number,
                isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age,
                isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code,
                isdn_author, isdn_shape, isdn_contents, isdn_barcode2,
                isdn_sample_image_url, isdn_useroption, isdn_external_links,
                id,
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
        let mut stmt =
            conn.prepare("SELECT id, ndl_id, name, transcription FROM authors ORDER BY id")?;
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

    pub fn update_book_cover_url(
        &self,
        id: i64,
        cover_url: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE books SET cover_url = ?1 WHERE id = ?2",
            params![cover_url, id],
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

    pub fn insert_copy(
        &self,
        book_id: i64,
        copy_type: &str,
        location: Option<&str>,
        notes: Option<&str>,
    ) -> Result<Copy, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO copies (book_id, copy_type, location, notes) VALUES (?1, ?2, ?3, ?4)",
            params![book_id, copy_type, location, notes],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Copy {
            id,
            book_id,
            copy_type: copy_type.to_string(),
            location: location.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
        })
    }

    pub fn list_copies(&self, book_id: i64) -> Result<Vec<CopyWithStatus>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT c.id, c.book_id, c.copy_type, c.location, c.notes,
                    b.name AS lent_to, lh.lent_date, lh.due_date
             FROM copies c
             LEFT JOIN lending_history lh ON lh.copy_id = c.id AND lh.returned_date IS NULL
             LEFT JOIN borrowers b ON b.id = lh.borrower_id
             WHERE c.book_id = ?1
             ORDER BY c.id",
        )?;
        let rows = stmt.query_map(params![book_id], |row| {
            Ok(CopyWithStatus {
                copy: Copy {
                    id: row.get(0)?,
                    book_id: row.get(1)?,
                    copy_type: row.get(2)?,
                    location: row.get(3)?,
                    notes: row.get(4)?,
                },
                lent_to: row.get(5)?,
                lent_date: row.get(6)?,
                due_date: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_copy(
        &self,
        id: i64,
        copy_type: Option<&str>,
        location: Option<&str>,
        notes: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE copies SET copy_type = COALESCE(?1, copy_type), location = ?2, notes = ?3 WHERE id = ?4",
            params![copy_type, location, notes, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_copy(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM copies WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn get_book_copy_counts(&self, book_id: i64) -> Result<(i64, i64), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN lh.returned_date IS NULL AND lh.id IS NOT NULL THEN 1 ELSE 0 END), 0)
             FROM copies c
             LEFT JOIN lending_history lh ON lh.copy_id = c.id AND lh.returned_date IS NULL
             WHERE c.book_id = ?1",
        )?;
        stmt.query_row(params![book_id], |row| Ok((row.get(0)?, row.get(1)?)))
    }

    pub fn insert_borrower(
        &self,
        name: &str,
        notes: Option<&str>,
    ) -> Result<Borrower, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO borrowers (name, notes) VALUES (?1, ?2)",
            params![name, notes],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Borrower {
            id,
            name: name.to_string(),
            notes: notes.map(|s| s.to_string()),
        })
    }

    pub fn list_borrowers(&self) -> Result<Vec<Borrower>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, notes FROM borrowers ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(Borrower {
                id: row.get(0)?,
                name: row.get(1)?,
                notes: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_borrower(
        &self,
        id: i64,
        name: Option<&str>,
        notes: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE borrowers SET name = COALESCE(?1, name), notes = ?2 WHERE id = ?3",
            params![name, notes, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_borrower(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM borrowers WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn lend_copy(
        &self,
        copy_id: i64,
        borrower_id: i64,
        lent_date: &str,
        due_date: Option<&str>,
        notes: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let existing = conn.query_row(
            "SELECT COUNT(*) FROM lending_history WHERE copy_id = ?1 AND returned_date IS NULL",
            params![copy_id],
            |row| row.get::<_, i64>(0),
        )?;
        if existing > 0 {
            return Err(rusqlite::Error::from(rusqlite::types::FromSqlError::InvalidType));
        }
        conn.execute(
            "INSERT INTO lending_history (copy_id, borrower_id, lent_date, due_date, notes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![copy_id, borrower_id, lent_date, due_date, notes],
        )?;
        Ok(())
    }

    pub fn return_copy(&self, copy_id: i64, returned_date: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE lending_history SET returned_date = ?1 WHERE copy_id = ?2 AND returned_date IS NULL",
            params![returned_date, copy_id],
        )?;
        Ok(affected > 0)
    }

    pub fn backup_to_file(&self, dst_path: &str) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut dst = Connection::open(dst_path)?;
        let backup = rusqlite::backup::Backup::new(&conn, &mut dst)?;
        backup.run_to_completion(5, std::time::Duration::from_millis(250), None)?;
        Ok(())
    }

    pub fn get_lending_history(
        &self,
        copy_id: i64,
    ) -> Result<Vec<LendingRecord>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT lh.id, lh.copy_id, lh.borrower_id, b.name, lh.lent_date, lh.due_date, lh.returned_date, lh.notes
             FROM lending_history lh
             LEFT JOIN borrowers b ON b.id = lh.borrower_id
             WHERE lh.copy_id = ?1
             ORDER BY lh.id DESC",
        )?;
        let rows = stmt.query_map(params![copy_id], |row| {
            Ok(LendingRecord {
                id: row.get(0)?,
                copy_id: row.get(1)?,
                borrower_id: row.get(2)?,
                borrower_name: row.get(3)?,
                lent_date: row.get(4)?,
                due_date: row.get(5)?,
                returned_date: row.get(6)?,
                notes: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok()
    }

    pub fn get_all_settings(&self) -> std::collections::HashMap<String, String> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings ORDER BY key")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn set_settings(
        &self,
        settings: &std::collections::HashMap<String, String>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        for (key, value) in settings {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        Ok(())
    }
}
