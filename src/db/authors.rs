use super::*;
use rusqlite::{OptionalExtension, Row, params};

impl Db {
    fn row_to_author(row: &Row<'_>) -> rusqlite::Result<Author> {
        Ok(Author {
            id: row.get(0)?,
            ndl_id: row.get(1)?,
            name: row.get(2)?,
            transcription: row.get(3)?,
        })
    }

    /// Find one author by normalized name (see
    /// `domain::author::normalize_author_name`). Scans the table in Rust
    /// because SQLite has no NFKC/whitespace folding; the authors table stays
    /// tiny (hundreds of rows), so this is cheaper than a migration.
    pub(crate) fn find_author_by_normalized_name(
        conn: &rusqlite::Connection,
        name: &str,
    ) -> Result<Option<(i64, Option<String>, Option<String>)>, rusqlite::Error> {
        let key = crate::domain::author::normalize_author_name(name);
        if key.is_empty() {
            return Ok(None);
        }
        let mut stmt = conn.prepare("SELECT id, ndl_id, transcription, name FROM authors")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (id, ndl_id, transcription, existing) = row?;
            if crate::domain::author::normalize_author_name(&existing) == key {
                return Ok(Some((id, ndl_id, transcription)));
            }
        }
        Ok(None)
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

        // Same person under another source's spelling (tag vs NDL vs manual)
        // must reuse the row instead of scattering.
        if let Some((id, existing_ndl, existing_transcription)) =
            Self::find_author_by_normalized_name(&conn, name)?
        {
            if existing_ndl.is_none() && ndl_id.is_some() {
                conn.execute(
                    "UPDATE authors SET ndl_id = ?1 WHERE id = ?2",
                    params![ndl_id, id],
                )?;
            }
            if existing_transcription.is_none() && transcription.is_some() {
                conn.execute(
                    "UPDATE authors SET transcription = ?1 WHERE id = ?2",
                    params![transcription, id],
                )?;
            }
            return Ok(id);
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

    /// Merge duplicate author rows into one survivor: reassign every
    /// book/CD/track link, backfill missing identity fields, delete dupes.
    /// Returns the number of removed rows. Links keep their sort order
    /// (survivor's own link wins on conflict).
    pub fn merge_authors(
        &self,
        survivor_id: i64,
        duplicate_ids: &[i64],
    ) -> Result<usize, rusqlite::Error> {
        let dupes: Vec<i64> = duplicate_ids
            .iter()
            .copied()
            .filter(|id| *id != survivor_id)
            .collect();
        if dupes.is_empty() {
            return Ok(0);
        }
        let placeholders = dupes.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let mut conn = self.0.lock().unwrap();
        let survivor_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM authors WHERE id = ?1)",
            params![survivor_id],
            |row| row.get(0),
        )?;
        if !survivor_exists {
            return Ok(0);
        }
        let tx = conn.transaction()?;
        for (table, entity) in [
            ("book_authors", "book_id"),
            ("cd_authors", "cd_id"),
            ("track_authors", "track_id"),
        ] {
            tx.execute(
                &format!(
                    "INSERT OR IGNORE INTO {table} ({entity}, author_id, sort_order) SELECT {entity}, ?1, MIN(sort_order) FROM {table} WHERE author_id IN ({placeholders}) GROUP BY {entity}"
                ),
                rusqlite::params_from_iter(
                    std::iter::once(survivor_id).chain(dupes.iter().copied()),
                ),
            )?;
            tx.execute(
                &format!("DELETE FROM {table} WHERE author_id IN ({placeholders})"),
                rusqlite::params_from_iter(dupes.iter().copied()),
            )?;
        }
        // Backfill identity fields the survivor lacks (first non-null wins).
        // Read first: the UNIQUE ndl_id slot must be freed before the
        // survivor can take it over.
        let backfill_ndl: Option<String> = tx.query_row(
            &format!(
                "SELECT ndl_id FROM authors WHERE id IN ({placeholders}) AND ndl_id IS NOT NULL ORDER BY id LIMIT 1"
            ),
            rusqlite::params_from_iter(dupes.iter().copied()),
            |row| row.get(0),
        ).optional()?;
        let backfill_transcription: Option<String> = tx.query_row(
            &format!(
                "SELECT transcription FROM authors WHERE id IN ({placeholders}) AND transcription IS NOT NULL ORDER BY id LIMIT 1"
            ),
            rusqlite::params_from_iter(dupes.iter().copied()),
            |row| row.get(0),
        ).optional()?;
        tx.execute(
            &format!("UPDATE authors SET ndl_id = NULL, transcription = NULL WHERE id IN ({placeholders})"),
            rusqlite::params_from_iter(dupes.iter().copied()),
        )?;
        tx.execute(
            "UPDATE authors SET ndl_id = COALESCE(ndl_id, ?1), transcription = COALESCE(transcription, ?2) WHERE id = ?3",
            params![backfill_ndl, backfill_transcription, survivor_id],
        )?;
        tx.execute(
            &format!("DELETE FROM authors WHERE id IN ({placeholders})"),
            rusqlite::params_from_iter(dupes.iter().copied()),
        )?;
        tx.commit()?;
        Ok(dupes.len())
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
    #[test]
    fn merge_authors_reassigns_links_backfills_and_deletes_dupes() {
        let db = Db::new(":memory:").expect("database");
        let survivor = db
            .create_author("Same Artist", None, None)
            .expect("author")
            .id;
        let dupe = db
            .create_author("Same　Artist", Some("セイム"), Some("ndl-9"))
            .expect("author")
            .id;
        let cd = db
            .insert_cd(&crate::db::NewCd {
                jan: None,
                title: "Test CD".to_string(),
                artist: None,
                publisher: None,
                label: None,
                catalog_number: None,
                publish_date: None,
                cover_url: None,
                description: None,
                disc_count: None,
                volume: None,
                tracks: None,
                parent_book_id: None,
                media_type: None,
                series_id: None,
            })
            .expect("CD");
        db.add_cd_author(cd.id, dupe).expect("link dupe");
        let merged = db.merge_authors(survivor, &[dupe]).expect("merge");
        assert_eq!(merged, 1);
        let authors = db.get_cd_authors(cd.id).expect("links");
        assert_eq!(
            authors.iter().map(|author| author.id).collect::<Vec<_>>(),
            vec![survivor]
        );
        let kept = db.get_author_by_id(survivor).expect("read").expect("row");
        assert_eq!(kept.ndl_id.as_deref(), Some("ndl-9"));
        assert_eq!(kept.transcription.as_deref(), Some("セイム"));
        assert!(db.get_author_by_id(dupe).expect("read").is_none());
        // Merging into a missing survivor removes nothing.
        assert_eq!(db.merge_authors(9999, &[survivor]).expect("noop"), 0);
        assert!(db.get_author_by_id(survivor).expect("read").is_some());
    }

    #[test]
    fn insert_author_reuses_normalized_name_and_backfills_ndl_id() {
        let db = Db::new(":memory:").expect("database");
        let first = db.insert_author(None, "村上　春樹", None).expect("insert");
        // Full-width space variant must reuse the row, not scatter.
        let second = db.insert_author(None, "村上 春樹", None).expect("reuse");
        assert_eq!(first, second);
        // Learning the NDL id later backfills the same row.
        let third = db
            .insert_author(Some("ndl-1"), "村上春樹", None)
            .expect("backfill");
        assert_eq!(first, third);
        let author = db.get_author_by_id(first).expect("read").expect("row");
        assert_eq!(author.ndl_id.as_deref(), Some("ndl-1"));
        // NDL-first lookup still resolves to the unified row.
        assert_eq!(
            db.insert_author(Some("ndl-1"), "M. Haruki", None)
                .expect("ndl"),
            first
        );
    }
}
