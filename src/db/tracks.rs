use super::*;
use rusqlite::params;

impl Db {
    pub fn insert_track(&self, book_id: i64, track: &NewTrack) -> Result<Track, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO tracks (book_id, cd_id, disc_number, track_number, title, duration) VALUES (?1, NULL, ?2, ?3, ?4, ?5)",
            params![book_id, track.disc_number.unwrap_or(1), track.track_number, track.title, track.duration],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Track {
            id,
            book_id,
            cd_id: None,
            disc_number: track.disc_number.unwrap_or(1),
            track_number: track.track_number,
            title: track.title.clone(),
            duration: track.duration.clone(),
            file_hash: None,
            file_name: None,
        })
    }

    pub fn insert_track_for_cd(&self, cd_id: i64, track: &NewTrack) -> Result<Track, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO tracks (book_id, cd_id, disc_number, track_number, title, duration) VALUES (NULL, ?1, ?2, ?3, ?4, ?5)",
            params![cd_id, track.disc_number.unwrap_or(1), track.track_number, track.title, track.duration],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Track {
            id,
            book_id: 0,
            cd_id: Some(cd_id),
            disc_number: track.disc_number.unwrap_or(1),
            track_number: track.track_number,
            title: track.title.clone(),
            duration: track.duration.clone(),
            file_hash: None,
            file_name: None,
        })
    }

    pub fn insert_tracks_batch(
        &self,
        book_id: i64,
        tracks: &[NewTrack],
    ) -> Result<Vec<Track>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut result = Vec::new();
        for track in tracks {
            conn.execute(
                "INSERT INTO tracks (book_id, cd_id, disc_number, track_number, title, duration) VALUES (?1, NULL, ?2, ?3, ?4, ?5)",
                params![book_id, track.disc_number.unwrap_or(1), track.track_number, track.title, track.duration],
            )?;
            let id = conn.last_insert_rowid();
            result.push(Track {
                id,
                book_id,
                cd_id: None,
                disc_number: track.disc_number.unwrap_or(1),
                track_number: track.track_number,
                title: track.title.clone(),
                duration: track.duration.clone(),
                file_hash: None,
                file_name: None,
            });
        }
        Ok(result)
    }

    pub fn insert_tracks_batch_for_cd(
        &self,
        cd_id: i64,
        tracks: &[NewTrack],
    ) -> Result<Vec<Track>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut result = Vec::new();
        for track in tracks {
            conn.execute(
                "INSERT INTO tracks (book_id, cd_id, disc_number, track_number, title, duration) VALUES (NULL, ?1, ?2, ?3, ?4, ?5)",
                params![cd_id, track.disc_number.unwrap_or(1), track.track_number, track.title, track.duration],
            )?;
            let id = conn.last_insert_rowid();
            result.push(Track {
                id,
                book_id: 0,
                cd_id: Some(cd_id),
                disc_number: track.disc_number.unwrap_or(1),
                track_number: track.track_number,
                title: track.title.clone(),
                duration: track.duration.clone(),
                file_hash: None,
                file_name: None,
            });
        }
        Ok(result)
    }

    pub fn list_tracks(&self, book_id: i64) -> Result<Vec<Track>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, book_id, cd_id, disc_number, track_number, title, duration, file_hash, file_name FROM tracks WHERE book_id = ?1 ORDER BY disc_number, track_number",
        )?;
        let rows = stmt.query_map(params![book_id], |row| {
            Ok(Track {
                id: row.get(0)?,
                book_id: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                cd_id: row.get(2)?,
                disc_number: row.get(3)?,
                track_number: row.get(4)?,
                title: row.get(5)?,
                duration: row.get(6)?,
                file_hash: row.get(7)?,
                file_name: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_tracks_for_cd(&self, cd_id: i64) -> Result<Vec<Track>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, book_id, cd_id, disc_number, track_number, title, duration, file_hash, file_name FROM tracks WHERE cd_id = ?1 ORDER BY disc_number, track_number",
        )?;
        let rows = stmt.query_map(params![cd_id], |row| {
            Ok(Track {
                id: row.get(0)?,
                book_id: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                cd_id: row.get(2)?,
                disc_number: row.get(3)?,
                track_number: row.get(4)?,
                title: row.get(5)?,
                duration: row.get(6)?,
                file_hash: row.get(7)?,
                file_name: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_track(
        &self,
        id: i64,
        title: Option<&str>,
        duration: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE tracks SET title = COALESCE(?1, title), duration = COALESCE(?2, duration) WHERE id = ?3",
            params![title, duration, id],
        )?;
        Ok(affected > 0)
    }

    pub fn update_track_audio(
        &self,
        id: i64,
        file_hash: Option<&str>,
        file_name: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE tracks SET file_hash = ?1, file_name = ?2 WHERE id = ?3",
            params![file_hash, file_name, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_track(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn update_track_position(
        &self,
        id: i64,
        disc_number: i64,
        track_number: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE tracks SET disc_number = ?1, track_number = ?2 WHERE id = ?3",
            params![disc_number, track_number, id],
        )?;
        Ok(affected > 0)
    }

    pub fn delete_tracks_by_book(&self, book_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM tracks WHERE book_id = ?1", params![book_id])?;
        Ok(())
    }

    pub fn delete_tracks_by_cd(&self, cd_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM tracks WHERE cd_id = ?1", params![cd_id])?;
        Ok(())
    }
}
