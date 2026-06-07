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
            "SELECT t.id, t.cd_id, t.book_id, t.title,
                    COALESCE(tm.artist, cdm.artist) AS artist,
                    COALESCE(tm.album,  cdm.album)  AS album,
                    COALESCE(tm.year,   cdm.year)   AS year,
                    COALESCE(tm.track_number, t.track_number) AS track_number,
                    COALESCE(tm.disc_number,  t.disc_number)  AS disc_number
             FROM tracks t
             LEFT JOIN track_metadata tm ON tm.track_id = t.id
             LEFT JOIN cd_metadata cdm    ON cdm.cd_id   = t.cd_id
             WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(a) = artist {
            sql.push_str(" AND (tm.artist LIKE ? OR cdm.artist LIKE ?)");
            let pat = format!("%{}%", a);
            params_vec.push(Box::new(pat.clone()));
            params_vec.push(Box::new(pat));
        }
        if let Some(a) = album {
            sql.push_str(" AND (tm.album LIKE ? OR cdm.album LIKE ?)");
            let pat = format!("%{}%", a);
            params_vec.push(Box::new(pat.clone()));
            params_vec.push(Box::new(pat));
        }
        if let Some(y) = year {
            sql.push_str(" AND (tm.year = ? OR cdm.year = ?)");
            params_vec.push(Box::new(y));
            params_vec.push(Box::new(y));
        }
        sql.push_str(" ORDER BY year DESC, title LIMIT ?");
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
