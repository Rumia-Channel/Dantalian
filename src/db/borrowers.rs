use super::*;
use rusqlite::params;

impl Db {
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
}
