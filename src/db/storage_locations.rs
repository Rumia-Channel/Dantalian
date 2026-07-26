use super::*;
use rusqlite::params;

impl Db {
    pub fn create_storage_location(
        &self,
        name: &str,
        parent_id: Option<i64>,
    ) -> Result<StorageLocation, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO storage_locations (name, parent_id) VALUES (?1, ?2)",
            params![name, parent_id],
        )?;
        let id = conn.last_insert_rowid();
        Ok(StorageLocation {
            id,
            name: name.to_string(),
            parent_id,
        })
    }

    pub fn list_storage_locations(&self) -> Result<Vec<StorageLocation>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id FROM storage_locations ORDER BY parent_id IS NOT NULL, parent_id, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StorageLocation {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_storage_location(
        &self,
        id: i64,
    ) -> Result<Option<StorageLocation>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, name, parent_id FROM storage_locations WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(StorageLocation {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn rename_storage_location(&self, id: i64, name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE storage_locations SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(affected > 0)
    }

    pub fn set_storage_location_parent(
        &self,
        id: i64,
        parent_id: Option<i64>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE storage_locations SET parent_id = ?1 WHERE id = ?2",
            params![parent_id, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_storage_location(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM storage_locations WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }
}
