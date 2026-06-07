use super::Db;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataSearchResult {
    pub track_id: i64,
    pub cd_id: Option<i64>,
    pub book_id: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i64>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
}

impl Db {
    pub fn search_tracks_by_metadata(
        &self,
        artist: Option<&str>,
        album: Option<&str>,
        year: Option<i64>,
        limit: i64,
    ) -> Result<Vec<MetadataSearchResult>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut sql = String::from(
            "SELECT track_id, cd_id, book_id, title, artist, album, year, track_number, disc_number
             FROM track_metadata WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(a) = artist {
            sql.push_str(" AND artist LIKE ?");
            params_vec.push(Box::new(format!("%{}%", a)));
        }
        if let Some(a) = album {
            sql.push_str(" AND album LIKE ?");
            params_vec.push(Box::new(format!("%{}%", a)));
        }
        if let Some(y) = year {
            sql.push_str(" AND year = ?");
            params_vec.push(Box::new(y));
        }
        sql.push_str(" ORDER BY year DESC NULLS LAST, title LIMIT ?");
        params_vec.push(Box::new(limit));

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(param_refs), |row| {
            Ok(MetadataSearchResult {
                track_id: row.get(0)?,
                cd_id: row.get(1)?,
                book_id: row.get(2)?,
                title: row.get(3)?,
                artist: row.get(4)?,
                album: row.get(5)?,
                year: row.get(6)?,
                track_number: row.get(7)?,
                disc_number: row.get(8)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }
}
