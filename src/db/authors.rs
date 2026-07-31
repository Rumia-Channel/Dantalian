use super::*;
use rusqlite::{Row, params};

impl Db {
    fn row_to_author(row: &Row<'_>) -> rusqlite::Result<Author> {
        Ok(Author {
            id: row.get(0)?,
            ndl_id: row.get(1)?,
            name: row.get(2)?,
            transcription: row.get(3)?,
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

    pub fn delete_author(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM authors WHERE id = ?1", params![id])?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleting_author_removes_relationships_but_keeps_tracks() {
        let db = Db::new(":memory:").expect("database");
        let author = db.create_author("Test artist", None, None).expect("author");
        let cd = db
            .insert_cd(&NewCd {
                jan: None,
                title: "Test CD".to_string(),
                artist: None,
                publisher: None,
                label: None,
                catalog_number: None,
                publish_date: None,
                cover_url: None,
                description: None,
                disc_count: Some(1),
                volume: None,
                tracks: None,
                parent_book_id: None,
                media_type: Some("cd".to_string()),
                series_id: None,
            })
            .expect("CD");
        let track = db
            .insert_track_for_cd(
                cd.id,
                &NewTrack {
                    disc_number: Some(1),
                    track_number: 1,
                    title: "Test track".to_string(),
                    duration: None,
                },
            )
            .expect("track");
        db.add_cd_author(cd.id, author.id).expect("CD author");
        db.add_track_author(track.id, author.id)
            .expect("track author");

        assert!(db.delete_author(author.id).expect("delete author"));
        assert!(
            db.get_author_by_id(author.id)
                .expect("author read")
                .is_none()
        );
        assert!(db.get_cd_authors(cd.id).expect("CD authors").is_empty());
        assert!(
            db.list_track_authors(track.id)
                .expect("track authors")
                .is_empty()
        );
        assert!(db.find_track_by_id(track.id).expect("track read").is_some());
        assert!(!db.delete_author(author.id).expect("delete missing author"));
    }
}
