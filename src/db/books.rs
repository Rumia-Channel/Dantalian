use super::*;
use rusqlite::{Connection, Row, params};

impl Db {
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
            jan: row.get(37)?,
            media_type: row.get(38)?,
            catalog_number: row.get(39)?,
            artist: row.get(40)?,
            label: row.get(41)?,
            disc_count: row.get(42)?,
            created_at: row.get(43)?,
            updated_at: row.get(44)?,
        })
    }

    pub fn insert_book(&self, book: &NewBook) -> Result<Book, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        conn.execute(
            "INSERT OR IGNORE INTO books (isbn, isdn, jan, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links, media_type, catalog_number, artist, label, disc_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41)",
            params![book.isbn, book.isdn, book.jan, book.title, book.publisher, book.publish_date, book.cover_url, book.description, book.title_transcription, book.series_title, book.series_title_transcription, book.alternative, book.alternative_transcription, book.volume, book.volume_transcription, book.price, book.extent, book.jpno, book.ndl_url, book.isdn_region, book.isdn_class, book.isdn_type, book.isdn_rating_gender, book.isdn_rating_age, book.isdn_genre_code, book.isdn_genre_name, book.isdn_genre_user, book.isdn_c_code, book.isdn_author, book.isdn_shape, book.isdn_contents, book.isdn_barcode2, book.isdn_sample_image_url, book.isdn_useroption, book.isdn_external_links, book.media_type, book.catalog_number, book.artist, book.label, book.disc_count, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Book {
            id,
            isbn: book.isbn.clone(),
            isdn: book.isdn.clone(),
            jan: book.jan.clone(),
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
            media_type: book.media_type.clone(),
            catalog_number: book.catalog_number.clone(),
            artist: book.artist.clone(),
            label: book.label.clone(),
            disc_count: book.disc_count,
            created_at: Some(now),
            updated_at: None,
        })
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

    pub fn list_books(&self) -> Result<Vec<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links, jan, media_type, catalog_number, artist, label, disc_count, created_at, updated_at FROM books ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_book)?;
        rows.collect()
    }

    pub fn find_by_isbn(&self, isbn: &str) -> Result<Option<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links, jan, media_type, catalog_number, artist, label, disc_count, created_at, updated_at FROM books WHERE isbn = ?1",
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
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links, jan, media_type, catalog_number, artist, label, disc_count, created_at, updated_at FROM books WHERE isdn = ?1",
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
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links, jan, media_type, catalog_number, artist, label, disc_count, created_at, updated_at FROM books WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::row_to_book)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn find_by_jan(&self, jan: &str) -> Result<Option<Book>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, isbn, isdn, title, publisher, publish_date, cover_url, description, title_transcription, series_title, series_title_transcription, alternative, alternative_transcription, volume, volume_transcription, price, extent, jpno, ndl_url, series_id, series_number, isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age, isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code, isdn_author, isdn_shape, isdn_contents, isdn_barcode2, isdn_sample_image_url, isdn_useroption, isdn_external_links, jan, media_type, catalog_number, artist, label, disc_count, created_at, updated_at FROM books WHERE jan = ?1",
        )?;
        let mut rows = stmt.query_map(params![jan], Self::row_to_book)?;
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
        jan: Option<&str>,
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
        media_type: Option<&str>,
        catalog_number: Option<&str>,
        artist: Option<&str>,
        label: Option<&str>,
        disc_count: Option<i64>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let affected = conn.execute(
            "UPDATE books SET isbn=?1, isdn=?2, jan=?3, title=?4, publisher=?5, publish_date=?6, description=?7,
             title_transcription=?8, series_title=?9, series_title_transcription=?10,
             alternative=?11, alternative_transcription=?12, volume=?13, volume_transcription=?14,
             price=?15, extent=?16, jpno=?17, ndl_url=?18,
             series_id=?19, series_number=?20,
             isdn_region=?21, isdn_class=?22, isdn_type=?23, isdn_rating_gender=?24, isdn_rating_age=?25,
             isdn_genre_code=?26, isdn_genre_name=?27, isdn_genre_user=?28, isdn_c_code=?29,
             isdn_author=?30, isdn_shape=?31, isdn_contents=?32, isdn_barcode2=?33,
             isdn_sample_image_url=?34, isdn_useroption=?35, isdn_external_links=?36,
             media_type=?37, catalog_number=?38, artist=?39, label=?40, disc_count=?41, updated_at=?42
             WHERE id=?43",
            params![
                isbn, isdn, jan, title, publisher, publish_date, description,
                title_transcription, series_title, series_title_transcription,
                alternative, alternative_transcription, volume, volume_transcription,
                price, extent, jpno, ndl_url,
                series_id, series_number,
                isdn_region, isdn_class, isdn_type, isdn_rating_gender, isdn_rating_age,
                isdn_genre_code, isdn_genre_name, isdn_genre_user, isdn_c_code,
                isdn_author, isdn_shape, isdn_contents, isdn_barcode2,
                isdn_sample_image_url, isdn_useroption, isdn_external_links,
                media_type, catalog_number, artist, label, disc_count, now,
                id,
            ],
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

    pub fn backup_to_file(&self, dst_path: &str) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut dst = Connection::open(dst_path)?;
        let backup = rusqlite::backup::Backup::new(&conn, &mut dst)?;
        backup.run_to_completion(5, std::time::Duration::from_millis(250), None)?;
        Ok(())
    }
}
