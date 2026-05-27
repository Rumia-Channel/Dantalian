use super::*;
use rusqlite::params;

impl Db {
    pub fn create_series(&self, name: &str) -> Result<Series, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("INSERT INTO series (name) VALUES (?1)", params![name])?;
        let id = conn.last_insert_rowid();
        Ok(Series {
            id,
            name: name.to_string(),
        })
    }

    pub fn list_series(&self) -> Result<Vec<Series>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name FROM series ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(Series {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn rename_series(&self, id: i64, name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE series SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_series(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM series WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn create_grand_series(&self, name: &str) -> Result<GrandSeries, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("INSERT INTO grand_series (name) VALUES (?1)", params![name])?;
        let id = conn.last_insert_rowid();
        Ok(GrandSeries {
            id,
            name: name.to_string(),
        })
    }

    pub fn list_grand_series(&self) -> Result<Vec<GrandSeriesWithItems>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT gs.id, gs.name, gsi.item_type, gsi.item_id,
                    COALESCE(s.name, b.title) AS item_name
             FROM grand_series gs
             LEFT JOIN grand_series_items gsi ON gs.id = gsi.grand_series_id
             LEFT JOIN series s ON gsi.item_type = 'series' AND s.id = gsi.item_id
             LEFT JOIN books b ON gsi.item_type = 'book' AND b.id = gsi.item_id
             ORDER BY gs.id, gsi.rowid",
        )?;
        let mut map: std::collections::HashMap<i64, GrandSeriesWithItems> =
            std::collections::HashMap::new();
        let rows = stmt.query_map([], |row| {
            let gs_id: i64 = row.get(0)?;
            let gs_name: String = row.get(1)?;
            let item_type: Option<String> = row.get(2)?;
            let item_id: Option<i64> = row.get(3)?;
            let item_name: Option<String> = row.get(4)?;
            Ok((gs_id, gs_name, item_type, item_id, item_name))
        })?;
        for row in rows {
            let (gs_id, gs_name, item_type, item_id, item_name) = row?;
            let entry = map.entry(gs_id).or_insert_with(|| GrandSeriesWithItems {
                id: gs_id,
                name: gs_name,
                items: Vec::new(),
            });
            if let (Some(it), Some(iid), Some(iname)) = (item_type, item_id, item_name) {
                entry.items.push(GrandSeriesItemInfo {
                    item_type: it,
                    item_id: iid,
                    name: iname,
                });
            }
        }
        Ok(map.into_values().collect())
    }

    pub fn rename_grand_series(&self, id: i64, name: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE grand_series SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_grand_series(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM grand_series WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn add_grand_series_item(
        &self,
        grand_series_id: i64,
        item_type: &str,
        item_id: i64,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO grand_series_items (grand_series_id, item_type, item_id) VALUES (?1, ?2, ?3)",
            params![grand_series_id, item_type, item_id],
        )?;
        Ok(())
    }

    pub fn remove_grand_series_item(
        &self,
        grand_series_id: i64,
        item_type: &str,
        item_id: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM grand_series_items WHERE grand_series_id = ?1 AND item_type = ?2 AND item_id = ?3",
            params![grand_series_id, item_type, item_id],
        )?;
        Ok(affected > 0)
    }
}
