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
        conn.execute(
            "INSERT OR IGNORE INTO cds (jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, parent_book_id, media_type, series_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![cd.jan, cd.title, cd.artist, cd.publisher, cd.label, cd.catalog_number, cd.publish_date, cd.cover_url, cd.description, cd.disc_count, cd.parent_book_id, cd.media_type, cd.series_id, now],
        )?;
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
            media_type: cd.media_type.clone(),
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
        cover_url: Option<&str>,
        description: Option<&str>,
        disc_count: Option<i64>,
        parent_book_id: Option<i64>,
        media_type: Option<&str>,
        series_id: Option<i64>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let affected = conn.execute(
            "UPDATE cds SET jan=?1, title=?2, artist=?3, publisher=?4, label=?5, catalog_number=?6, publish_date=?7, cover_url=?8, description=?9, disc_count=?10, parent_book_id=?11, media_type=?12, series_id=?13, updated_at=?14 WHERE id=?15",
            params![jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, parent_book_id, media_type, series_id, now, id],
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
}
