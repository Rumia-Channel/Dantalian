use super::*;
use rusqlite::{Row, params};

impl Db {
    fn row_to_cd(row: &Row<'_>) -> rusqlite::Result<Cd> {
        Ok(Cd {
            id: row.get(0)?,
            jan: row.get(1)?,
            title: row.get(2)?,
            artist: row.get(3)?,
            publisher: row.get(4)?,
            label: row.get(5)?,
            catalog_number: row.get(6)?,
            publish_date: row.get(7)?,
            cover_url: row.get(8)?,
            description: row.get(9)?,
            disc_count: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
            parent_book_id: row.get(13)?,
            media_type: row.get(14)?,
            series_id: row.get(15)?,
        })
    }

    pub fn insert_cd(&self, cd: &NewCd) -> Result<Cd, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let media_type = cd.media_type.clone().unwrap_or_else(|| "cd".to_string());
        let changes = conn.execute(
            "INSERT OR IGNORE INTO cds (jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, parent_book_id, media_type, series_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![cd.jan, cd.title, cd.artist, cd.publisher, cd.label, cd.catalog_number, cd.publish_date, cd.cover_url, cd.description, cd.disc_count, cd.parent_book_id, media_type, cd.series_id, now],
        )?;
        if changes == 0 {
            if let Some(jan) = &cd.jan {
                let mut stmt = conn.prepare(
                    "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE jan = ?1",
                )?;
                if let Some(row) = stmt.query_map(params![jan], Self::row_to_cd)?.next() {
                    return row;
                }
            }
            return Err(rusqlite::Error::ToSqlConversionFailure(
                Box::new(std::io::Error::new(std::io::ErrorKind::Other, "INSERT was ignored but existing CD not found"))
            ));
        }
        let id = conn.last_insert_rowid();
        Ok(Cd {
            id,
            jan: cd.jan.clone(),
            title: cd.title.clone(),
            artist: cd.artist.clone(),
            publisher: cd.publisher.clone(),
            label: cd.label.clone(),
            catalog_number: cd.catalog_number.clone(),
            publish_date: cd.publish_date.clone(),
            cover_url: cd.cover_url.clone(),
            description: cd.description.clone(),
            disc_count: cd.disc_count,
            created_at: Some(now),
            updated_at: None,
            parent_book_id: cd.parent_book_id,
            media_type: Some(media_type),
            series_id: cd.series_id,
        })
    }

    pub fn find_by_cd_jan(&self, jan: &str) -> Result<Option<Cd>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE jan = ?1",
        )?;
        let mut rows = stmt.query_map(params![jan], Self::row_to_cd)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_cds(&self) -> Result<Vec<Cd>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, created_at, updated_at, parent_book_id, media_type, series_id FROM cds ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_cd)?;
        rows.collect()
    }

    pub fn find_cd_by_id(&self, id: i64) -> Result<Option<Cd>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::row_to_cd)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn delete_cd(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM cds WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn update_cd(
        &self,
        id: i64,
        jan: Option<&str>,
        title: &str,
        artist: Option<&str>,
        publisher: Option<&str>,
        label: Option<&str>,
        catalog_number: Option<&str>,
        publish_date: Option<&str>,
        description: Option<&str>,
        disc_count: Option<i64>,
        parent_book_id: Option<i64>,
        media_type: Option<&str>,
        series_id: Option<i64>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let affected = conn.execute(
            "UPDATE cds SET jan=?1, title=?2, artist=?3, publisher=?4, label=?5, catalog_number=?6, publish_date=?7, description=?8, disc_count=?9, parent_book_id=?10, media_type=?11, series_id=?12, updated_at=?13 WHERE id=?14",
            params![jan, title, artist, publisher, label, catalog_number, publish_date, description, disc_count, parent_book_id, media_type, series_id, now, id],
        )?;
        Ok(affected > 0)
    }

    pub fn update_cd_cover_url(
        &self,
        id: i64,
        cover_url: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE cds SET cover_url = ?1 WHERE id = ?2",
            params![cover_url, id],
        )?;
        Ok(affected > 0)
    }

    pub fn find_cds_by_parent_book(&self, book_id: i64) -> Result<Vec<Cd>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE parent_book_id = ?1",
        )?;
        let rows = stmt.query_map(params![book_id], Self::row_to_cd)?;
        rows.collect()
    }

    pub fn add_cd_author(&self, cd_id: i64, author_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO cd_authors (cd_id, author_id) VALUES (?1, ?2)",
            params![cd_id, author_id],
        )?;
        Ok(())
    }

    pub fn remove_cd_author(&self, cd_id: i64, author_id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM cd_authors WHERE cd_id = ?1 AND author_id = ?2",
            params![cd_id, author_id],
        )?;
        Ok(affected > 0)
    }

    pub fn get_cd_authors(&self, cd_id: i64) -> Result<Vec<BookAuthor>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.ndl_id, a.name, a.transcription, ca.sort_order
             FROM authors a
             JOIN cd_authors ca ON a.id = ca.author_id
             WHERE ca.cd_id = ?1
             ORDER BY ca.sort_order, ca.author_id",
        )?;
        let rows = stmt.query_map(params![cd_id], |row| {
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

    pub fn update_cd_author_order(
        &self,
        cd_id: i64,
        author_id: i64,
        sort_order: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE cd_authors SET sort_order = ?1 WHERE cd_id = ?2 AND author_id = ?3",
            params![sort_order, cd_id, author_id],
        )?;
        Ok(affected > 0)
    }
}
