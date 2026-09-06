use super::Db;
use crate::db_models::BookAuthor;
use rusqlite::params;

impl Db {
    pub fn add_track_author(&self, track_id: i64, author_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM track_authors WHERE track_id = ?1",
                params![track_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT OR IGNORE INTO track_authors (track_id, author_id, sort_order) VALUES (?1, ?2, ?3)",
            params![track_id, author_id, sort_order],
        )?;
        Ok(())
    }

    pub fn remove_track_author(
        &self,
        track_id: i64,
        author_id: i64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM track_authors WHERE track_id = ?1 AND author_id = ?2",
            params![track_id, author_id],
        )?;
        Ok(())
    }

    pub fn update_track_author_order(
        &self,
        track_id: i64,
        author_id: i64,
        sort_order: i64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE track_authors SET sort_order = ?1 WHERE track_id = ?2 AND author_id = ?3",
            params![sort_order, track_id, author_id],
        )?;
        Ok(())
    }

    pub fn list_track_authors(&self, track_id: i64) -> Result<Vec<BookAuthor>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.name, a.transcription, a.ndl_id, ta.sort_order
             FROM track_authors ta
             JOIN authors a ON a.id = ta.author_id
             WHERE ta.track_id = ?1
             ORDER BY ta.sort_order ASC, a.id ASC",
        )?;
        let rows = stmt.query_map(params![track_id], |row| {
            Ok(BookAuthor {
                id: row.get(0)?,
                name: row.get(1)?,
                transcription: row.get(2)?,
                ndl_id: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn replace_track_authors(
        &self,
        track_id: i64,
        author_ids: &[i64],
    ) -> Result<(), rusqlite::Error> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM track_authors WHERE track_id = ?1",
            params![track_id],
        )?;
        for (i, aid) in author_ids.iter().enumerate() {
            tx.execute(
                "INSERT OR IGNORE INTO track_authors (track_id, author_id, sort_order) VALUES (?1, ?2, ?3)",
                params![track_id, aid, i as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn ensure_authors_for_names(&self, names: &[String]) -> Result<Vec<i64>, rusqlite::Error> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let mut ids = Vec::with_capacity(names.len());
        for name in names {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Byte-exact lookup scatters tag spellings ("A　B" vs "A B") into
            // duplicate rows; match on the normalized identity instead.
            let existing = Self::find_author_by_normalized_name(&tx, trimmed)?;
            let id = match existing {
                Some((id, _, _)) => id,
                None => {
                    tx.execute("INSERT INTO authors (name) VALUES (?1)", params![trimmed])?;
                    tx.last_insert_rowid()
                }
            };
            ids.push(id);
        }
        tx.commit()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::Db;

    #[test]
    fn ensure_reuses_spelling_variants_instead_of_scattering() {
        let db = Db::new(":memory:").expect("database");
        let ids = db
            .ensure_authors_for_names(&["村上　春樹".to_string(), "村上 春樹".to_string()])
            .expect("ensure");
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], ids[1]);
        // Exact repeat still resolves to the same row.
        let again = db
            .ensure_authors_for_names(&["村上春樹".to_string()])
            .expect("ensure");
        assert_eq!(again, vec![ids[0]]);
    }
}
