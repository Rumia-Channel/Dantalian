use super::*;
use rusqlite::params;

impl Db {
    pub fn create_label(&self, name: &str) -> Result<Label, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("INSERT INTO labels (name) VALUES (?1)", params![name])?;
        let id = conn.last_insert_rowid();
        Ok(Label {
            id,
            name: name.to_string(),
        })
    }

    pub fn get_or_create_label(&self, name: &str) -> Result<Label, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let existing: Option<Label> = conn
            .query_row(
                "SELECT id, name FROM labels WHERE name = ?1",
                params![name],
                |row| {
                    Ok(Label {
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                },
            )
            .ok();
        if let Some(label) = existing {
            return Ok(label);
        }
        conn.execute("INSERT INTO labels (name) VALUES (?1)", params![name])?;
        let id = conn.last_insert_rowid();
        Ok(Label {
            id,
            name: name.to_string(),
        })
    }

    pub fn list_labels(&self) -> Result<Vec<Label>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name FROM labels ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(Label {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn rename_label(&self, id: i64, name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE labels SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_label(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM labels WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }
}
