use super::*;
use rusqlite::params;

impl Db {
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
            return Err(rusqlite::Error::from(
                rusqlite::types::FromSqlError::InvalidType,
            ));
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

    pub fn get_lending_history(&self, copy_id: i64) -> Result<Vec<LendingRecord>, rusqlite::Error> {
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
}
