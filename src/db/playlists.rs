use super::*;
use rusqlite::{Connection, Row, params};
use std::collections::HashSet;

fn row_to_playlist(row: &Row<'_>) -> rusqlite::Result<Playlist> {
    Ok(Playlist {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        cover_cd_id: row.get(3)?,
        cover_url: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn playlist_select_sql() -> &'static str {
    "SELECT p.id, p.name, p.description, p.cover_cd_id, c.cover_url, p.created_at, p.updated_at
     FROM playlists p
     LEFT JOIN cds c ON c.id = p.cover_cd_id"
}

fn row_to_playlist_track(row: &Row<'_>) -> rusqlite::Result<PlaylistTrackEntry> {
    Ok(PlaylistTrackEntry {
        position: row.get(0)?,
        track: Track {
            id: row.get(1)?,
            book_id: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            cd_id: row.get(3)?,
            disc_number: row.get(4)?,
            track_number: row.get(5)?,
            title: row.get(6)?,
            duration: row.get(7)?,
            file_hash: row.get(8)?,
            file_name: row.get(9)?,
        },
        cd: Cd {
            id: row.get(10)?,
            jan: row.get(11)?,
            title: row.get(12)?,
            artist: row.get(13)?,
            publisher: row.get(14)?,
            label: row.get(15)?,
            catalog_number: row.get(16)?,
            publish_date: row.get(17)?,
            cover_url: row.get(18)?,
            description: row.get(19)?,
            disc_count: row.get(20)?,
            volume: row.get(21)?,
            created_at: row.get(22)?,
            updated_at: row.get(23)?,
            parent_book_id: row.get(24)?,
            media_type: row.get(25)?,
            series_id: row.get(26)?,
        },
    })
}

fn list_playlist_tracks_with_conn(
    conn: &Connection,
    playlist_id: i64,
) -> Result<Vec<PlaylistTrackEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT pt.position,
                t.id, t.book_id, t.cd_id, t.disc_number, t.track_number,
                t.title, t.duration, t.file_hash, t.file_name,
                c.id, c.jan, c.title, c.artist, c.publisher, c.label,
                c.catalog_number, c.publish_date, c.cover_url, c.description,
                c.disc_count, c.volume, c.created_at, c.updated_at,
                c.parent_book_id, c.media_type, c.series_id
         FROM playlist_tracks pt
         JOIN tracks t ON t.id = pt.track_id
         JOIN cds c ON c.id = t.cd_id
         WHERE pt.playlist_id = ?1
         ORDER BY pt.position ASC, pt.rowid ASC",
    )?;
    let rows = stmt.query_map(params![playlist_id], row_to_playlist_track)?;
    rows.collect()
}

impl Db {
    pub fn list_playlists(&self) -> Result<Vec<PlaylistWithTracks>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(&format!("{} ORDER BY p.id DESC", playlist_select_sql()))?;
        let playlists = stmt
            .query_map([], row_to_playlist)?
            .collect::<Result<Vec<_>, _>>()?;
        playlists
            .into_iter()
            .map(|playlist| {
                let tracks = list_playlist_tracks_with_conn(&conn, playlist.id)?;
                Ok(PlaylistWithTracks { playlist, tracks })
            })
            .collect()
    }

    pub fn find_playlist_by_id(
        &self,
        id: i64,
    ) -> Result<Option<PlaylistWithTracks>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(&format!("{} WHERE p.id = ?1", playlist_select_sql()))?;
        let playlist = stmt
            .query_map(params![id], row_to_playlist)?
            .next()
            .transpose()?;
        playlist
            .map(|playlist| {
                let tracks = list_playlist_tracks_with_conn(&conn, playlist.id)?;
                Ok(PlaylistWithTracks { playlist, tracks })
            })
            .transpose()
    }

    pub fn insert_playlist(
        &self,
        name: &str,
        description: Option<&str>,
        cover_cd_id: Option<i64>,
    ) -> Result<Playlist, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        conn.execute(
            "INSERT INTO playlists (name, description, cover_cd_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![name, description, cover_cd_id, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Playlist {
            id,
            name: name.to_string(),
            description: description.map(str::to_string),
            cover_cd_id,
            cover_url: cover_cd_id.and_then(|cd_id| {
                conn.query_row(
                    "SELECT cover_url FROM cds WHERE id = ?1",
                    params![cd_id],
                    |row| row.get(0),
                )
                .ok()
            }),
            created_at: Some(now.clone()),
            updated_at: Some(now),
        })
    }

    pub fn update_playlist(
        &self,
        id: i64,
        name: &str,
        description: Option<&str>,
        cover_cd_id: Option<i64>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let affected = conn.execute(
            "UPDATE playlists
             SET name = ?1, description = ?2, cover_cd_id = ?3, updated_at = ?4
             WHERE id = ?5",
            params![name, description, cover_cd_id, now, id],
        )?;
        Ok(affected > 0)
    }

    pub fn update_playlist_with_tracks(
        &self,
        id: i64,
        name: &str,
        description: Option<&str>,
        cover_cd_id: Option<i64>,
        track_ids: &[i64],
    ) -> Result<bool, rusqlite::Error> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let affected = tx.execute(
            "UPDATE playlists
             SET name = ?1, description = ?2, cover_cd_id = ?3, updated_at = ?4
             WHERE id = ?5",
            params![name, description, cover_cd_id, now, id],
        )?;
        if affected == 0 {
            return Ok(false);
        }

        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            params![id],
        )?;
        let mut seen = HashSet::new();
        for (position, track_id) in track_ids.iter().enumerate() {
            if !seen.insert(*track_id) {
                continue;
            }
            let valid: bool = tx.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM tracks
                     WHERE id = ?1 AND cd_id IS NOT NULL AND file_hash IS NOT NULL
                 )",
                params![track_id],
                |row| row.get(0),
            )?;
            if !valid {
                return Ok(false);
            }
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position)
                 VALUES (?1, ?2, ?3)",
                params![id, track_id, position as i64],
            )?;
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn delete_playlist(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn add_playlist_track(
        &self,
        playlist_id: i64,
        track_id: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let valid: bool = conn.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM tracks
                 WHERE id = ?1 AND cd_id IS NOT NULL AND file_hash IS NOT NULL
             ) AND EXISTS(SELECT 1 FROM playlists WHERE id = ?2)",
            params![track_id, playlist_id],
            |row| row.get(0),
        )?;
        if !valid {
            return Ok(false);
        }
        let position: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position)
             VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, position],
        )?;
        Ok(true)
    }

    pub fn remove_playlist_track(
        &self,
        playlist_id: i64,
        track_id: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let removed_position: Option<i64> = conn
            .query_row(
                "SELECT position FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
                params![playlist_id, track_id],
                |row| row.get(0),
            )
            .ok();
        let affected = conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![playlist_id, track_id],
        )?;
        if let Some(position) = removed_position {
            conn.execute(
                "UPDATE playlist_tracks
                 SET position = position - 1
                 WHERE playlist_id = ?1 AND position > ?2",
                params![playlist_id, position],
            )?;
        }
        Ok(affected > 0)
    }

    pub fn set_playlist_tracks(
        &self,
        playlist_id: i64,
        track_ids: &[i64],
    ) -> Result<bool, rusqlite::Error> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
            params![playlist_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(false);
        }

        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
        )?;
        let mut seen = HashSet::new();
        let mut position = 0i64;
        for track_id in track_ids {
            if !seen.insert(*track_id) {
                continue;
            }
            let valid: bool = tx.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM tracks
                     WHERE id = ?1 AND cd_id IS NOT NULL AND file_hash IS NOT NULL
                 )",
                params![track_id],
                |row| row.get(0),
            )?;
            if !valid {
                return Ok(false);
            }
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position)
                 VALUES (?1, ?2, ?3)",
                params![playlist_id, track_id, position],
            )?;
            position += 1;
        }
        tx.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_crud_and_track_order() {
        let db = Db::new(":memory:").expect("database");
        let cd = db
            .insert_cd(&NewCd {
                jan: None,
                title: "Test album".to_string(),
                artist: Some("Test artist".to_string()),
                publisher: None,
                label: None,
                catalog_number: None,
                publish_date: None,
                cover_url: Some("test-cover.jpg".to_string()),
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
                    duration: Some("1:00".to_string()),
                },
            )
            .expect("track");
        db.update_track_audio(track.id, Some("hash"), Some("test.mp3"))
            .expect("audio");
        let second_track = db
            .insert_track_for_cd(
                cd.id,
                &NewTrack {
                    disc_number: Some(1),
                    track_number: 2,
                    title: "Second track".to_string(),
                    duration: Some("2:00".to_string()),
                },
            )
            .expect("second track");
        db.update_track_audio(second_track.id, Some("hash-2"), Some("test-2.mp3"))
            .expect("second audio");
        let third_track = db
            .insert_track_for_cd(
                cd.id,
                &NewTrack {
                    disc_number: Some(1),
                    track_number: 3,
                    title: "Third track".to_string(),
                    duration: Some("3:00".to_string()),
                },
            )
            .expect("third track");
        db.update_track_audio(third_track.id, Some("hash-3"), Some("test-3.mp3"))
            .expect("third audio");

        let playlist = db
            .insert_playlist("Favorites", Some("Test playlist"), Some(cd.id))
            .expect("playlist");
        assert_eq!(playlist.cover_url.as_deref(), Some("test-cover.jpg"));
        assert!(db.add_playlist_track(playlist.id, track.id).unwrap());
        assert!(db.add_playlist_track(playlist.id, second_track.id).unwrap());
        assert!(db.add_playlist_track(playlist.id, third_track.id).unwrap());
        let loaded = db.find_playlist_by_id(playlist.id).unwrap().unwrap();
        assert_eq!(loaded.tracks.len(), 3);
        assert_eq!(
            loaded
                .tracks
                .iter()
                .map(|entry| entry.track.id)
                .collect::<Vec<_>>(),
            vec![track.id, second_track.id, third_track.id]
        );
        assert!(
            db.set_playlist_tracks(playlist.id, &[third_track.id, track.id, second_track.id])
                .unwrap()
        );
        let loaded = db.find_playlist_by_id(playlist.id).unwrap().unwrap();
        assert_eq!(
            loaded
                .tracks
                .iter()
                .map(|entry| entry.track.id)
                .collect::<Vec<_>>(),
            vec![third_track.id, track.id, second_track.id]
        );
        assert!(
            db.remove_playlist_track(playlist.id, second_track.id)
                .unwrap()
        );
        let loaded = db.find_playlist_by_id(playlist.id).unwrap().unwrap();
        assert_eq!(loaded.tracks.len(), 2);
        assert_eq!(loaded.tracks[0].track.id, third_track.id);
        assert_eq!(loaded.tracks[1].track.id, track.id);

        assert!(
            db.set_playlist_tracks(playlist.id, &[track.id, track.id])
                .unwrap()
        );

        let loaded = db.find_playlist_by_id(playlist.id).unwrap().unwrap();
        assert_eq!(loaded.tracks.len(), 1);
        assert_eq!(loaded.tracks[0].track.id, track.id);
        assert_eq!(loaded.tracks[0].cd.id, cd.id);
        assert!(db.remove_playlist_track(playlist.id, track.id).unwrap());
        assert!(db.delete_playlist(playlist.id).unwrap());
    }
}
