use super::*;
use crate::external::audio_meta::TrackMetadata;
use rusqlite::{Connection, OptionalExtension, Row, params};

fn update_cd_conn(
    conn: &Connection,
    id: i64,
    jan: Option<&str>,
    title: &str,
    artist: Option<&str>,
    publisher: Option<&str>,
    label: Option<&str>,
    catalog_number: Option<&str>,
    publish_date: Option<&str>,
    description: Option<&str>,
    disc_count: Option<i64>,
    volume: Option<&str>,
    parent_book_id: Option<i64>,
    media_type: Option<&str>,
    series_id: Option<i64>,
) -> Result<bool, rusqlite::Error> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let affected = conn.execute(
        "UPDATE cds SET jan=?1, title=?2, artist=?3, publisher=?4, label=?5, catalog_number=?6, publish_date=?7, description=?8, disc_count=?9, volume=?10, parent_book_id=?11, media_type=?12, series_id=?13, updated_at=?14 WHERE id=?15",
        params![jan, title, artist, publisher, label, catalog_number, publish_date, description, disc_count, volume, parent_book_id, media_type, series_id, now, id],
    )?;
    Ok(affected > 0)
}

impl Db {
    fn row_to_cd(row: &Row<'_>) -> rusqlite::Result<Cd> {
        Ok(Cd {
            id: row.get(0)?,
            jan: row.get(1)?,
            title: row.get(2)?,
            artist: row.get(3)?,
            publisher: row.get(4)?,
            label: row.get(5)?,
            catalog_number: row.get(6)?,
            publish_date: row.get(7)?,
            cover_url: row.get(8)?,
            description: row.get(9)?,
            disc_count: row.get(10)?,
            volume: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
            parent_book_id: row.get(14)?,
            media_type: row.get(15)?,
            series_id: row.get(16)?,
        })
    }

    pub fn insert_cd(&self, cd: &NewCd) -> Result<Cd, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let media_type = cd.media_type.clone().unwrap_or_else(|| "cd".to_string());
        let changes = conn.execute(
            "INSERT OR IGNORE INTO cds (jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, parent_book_id, media_type, series_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![cd.jan, cd.title, cd.artist, cd.publisher, cd.label, cd.catalog_number, cd.publish_date, cd.cover_url, cd.description, cd.disc_count, cd.volume, cd.parent_book_id, media_type, cd.series_id, now],
        )?;
        if changes == 0 {
            if let Some(jan) = &cd.jan {
                let mut stmt = conn.prepare(
                    "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE jan = ?1",
                )?;
                if let Some(row) = stmt.query_map(params![jan], Self::row_to_cd)?.next() {
                    return row;
                }
            }
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "INSERT was ignored but existing CD not found",
                ),
            )));
        }
        let id = conn.last_insert_rowid();
        Ok(Cd {
            id,
            jan: cd.jan.clone(),
            title: cd.title.clone(),
            artist: cd.artist.clone(),
            publisher: cd.publisher.clone(),
            label: cd.label.clone(),
            catalog_number: cd.catalog_number.clone(),
            publish_date: cd.publish_date.clone(),
            cover_url: cd.cover_url.clone(),
            description: cd.description.clone(),
            disc_count: cd.disc_count,
            volume: cd.volume.clone(),
            created_at: Some(now),
            updated_at: None,
            parent_book_id: cd.parent_book_id,
            media_type: Some(media_type),
            series_id: cd.series_id,
        })
    }

    /// CD本体と、手動登録時に同時に確定する関連データを1トランザクションで作成する。
    pub fn insert_manual_cd(
        &self,
        cd: &NewCd,
        author_ids: &[i64],
        tracks: &[(NewTrack, Option<TrackMetadata>)],
        album_metadata: Option<&CdMetadata>,
        grand_series_id: Option<i64>,
    ) -> Result<Cd, rusqlite::Error> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let media_type = cd.media_type.clone().unwrap_or_else(|| "cd".to_string());

        tx.execute(
            "INSERT INTO cds (jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, parent_book_id, media_type, series_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![cd.jan, cd.title, cd.artist, cd.publisher, cd.label, cd.catalog_number, cd.publish_date, cd.cover_url, cd.description, cd.disc_count, cd.volume, cd.parent_book_id, media_type, cd.series_id, now],
        )?;
        let cd_id = tx.last_insert_rowid();

        for author_id in author_ids {
            tx.execute(
                "INSERT INTO cd_authors (cd_id, author_id) VALUES (?1, ?2)",
                params![cd_id, author_id],
            )?;
        }

        for (track, metadata) in tracks {
            tx.execute(
                "INSERT INTO tracks (book_id, cd_id, disc_number, track_number, title, duration) VALUES (NULL, ?1, ?2, ?3, ?4, ?5)",
                params![cd_id, track.disc_number.unwrap_or(1), track.track_number, track.title, track.duration],
            )?;
            let track_id = tx.last_insert_rowid();
            if let Some(metadata) = metadata {
                crate::db::track_metadata::upsert_track_metadata_conn(&tx, track_id, metadata)?;
            }
        }

        if let Some(metadata) = album_metadata {
            let mut metadata = metadata.clone();
            metadata.cd_id = cd_id;
            crate::db::cd_metadata::upsert_cd_metadata_conn(&tx, cd_id, &metadata)?;
        }

        if let Some(grand_series_id) = grand_series_id {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM grand_series WHERE id = ?1)",
                params![grand_series_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            tx.execute(
                "INSERT INTO grand_series_items (grand_series_id, item_type, item_id) VALUES (?1, 'cd', ?2)",
                params![grand_series_id, cd_id],
            )?;
        }

        tx.commit()?;
        Ok(Cd {
            id: cd_id,
            jan: cd.jan.clone(),
            title: cd.title.clone(),
            artist: cd.artist.clone(),
            publisher: cd.publisher.clone(),
            label: cd.label.clone(),
            catalog_number: cd.catalog_number.clone(),
            publish_date: cd.publish_date.clone(),
            cover_url: cd.cover_url.clone(),
            description: cd.description.clone(),
            disc_count: cd.disc_count,
            volume: cd.volume.clone(),
            created_at: Some(now),
            updated_at: None,
            parent_book_id: cd.parent_book_id,
            media_type: Some(media_type),
            series_id: cd.series_id,
        })
    }

    pub fn find_by_cd_jan(&self, jan: &str) -> Result<Option<Cd>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE jan = ?1",
        )?;
        let mut rows = stmt.query_map(params![jan], Self::row_to_cd)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_cds(&self) -> Result<Vec<Cd>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, created_at, updated_at, parent_book_id, media_type, series_id FROM cds ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_cd)?;
        rows.collect()
    }

    pub fn find_cd_by_id(&self, id: i64) -> Result<Option<Cd>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::row_to_cd)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn delete_cd(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute("DELETE FROM cds WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn update_cd(
        &self,
        id: i64,
        jan: Option<&str>,
        title: &str,
        artist: Option<&str>,
        publisher: Option<&str>,
        label: Option<&str>,
        catalog_number: Option<&str>,
        publish_date: Option<&str>,
        description: Option<&str>,
        disc_count: Option<i64>,
        volume: Option<&str>,
        parent_book_id: Option<i64>,
        media_type: Option<&str>,
        series_id: Option<i64>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        update_cd_conn(
            &conn,
            id,
            jan,
            title,
            artist,
            publisher,
            label,
            catalog_number,
            publish_date,
            description,
            disc_count,
            volume,
            parent_book_id,
            media_type,
            series_id,
        )
    }

    pub fn update_cd_with_metadata(
        &self,
        id: i64,
        jan: Option<&str>,
        title: &str,
        artist: Option<&str>,
        publisher: Option<&str>,
        label: Option<&str>,
        catalog_number: Option<&str>,
        publish_date: Option<&str>,
        description: Option<&str>,
        disc_count: Option<i64>,
        volume: Option<&str>,
        parent_book_id: Option<i64>,
        media_type: Option<&str>,
        series_id: Option<i64>,
        metadata: Option<&CdMetadata>,
    ) -> Result<bool, rusqlite::Error> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let affected = update_cd_conn(
            &tx,
            id,
            jan,
            title,
            artist,
            publisher,
            label,
            catalog_number,
            publish_date,
            description,
            disc_count,
            volume,
            parent_book_id,
            media_type,
            series_id,
        )?;
        if let Some(metadata) = metadata {
            crate::db::cd_metadata::upsert_cd_metadata_conn(&tx, id, metadata)?;
        }
        tx.commit()?;
        Ok(affected)
    }

    pub fn update_cd_cover_url(
        &self,
        id: i64,
        cover_url: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE cds SET cover_url = ?1 WHERE id = ?2",
            params![cover_url, id],
        )?;
        Ok(affected > 0)
    }

    pub fn find_cds_by_parent_book(&self, book_id: i64) -> Result<Vec<Cd>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, jan, title, artist, publisher, label, catalog_number, publish_date, cover_url, description, disc_count, volume, created_at, updated_at, parent_book_id, media_type, series_id FROM cds WHERE parent_book_id = ?1",
        )?;
        let rows = stmt.query_map(params![book_id], Self::row_to_cd)?;
        rows.collect()
    }

    pub fn add_cd_author(&self, cd_id: i64, author_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO cd_authors (cd_id, author_id) VALUES (?1, ?2)",
            params![cd_id, author_id],
        )?;
        Ok(())
    }

    pub fn remove_cd_author(&self, cd_id: i64, author_id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM cd_authors WHERE cd_id = ?1 AND author_id = ?2",
            params![cd_id, author_id],
        )?;
        Ok(affected > 0)
    }

    pub fn get_cd_authors(&self, cd_id: i64) -> Result<Vec<BookAuthor>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.ndl_id, a.name, a.transcription, ca.sort_order
             FROM authors a
             JOIN cd_authors ca ON a.id = ca.author_id
             WHERE ca.cd_id = ?1
             ORDER BY ca.sort_order, ca.author_id",
        )?;
        let rows = stmt.query_map(params![cd_id], |row| {
            Ok(BookAuthor {
                id: row.get(0)?,
                ndl_id: row.get(1)?,
                name: row.get(2)?,
                transcription: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_cd_author_order(
        &self,
        cd_id: i64,
        author_id: i64,
        sort_order: i64,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let affected = conn.execute(
            "UPDATE cd_authors SET sort_order = ?1 WHERE cd_id = ?2 AND author_id = ?3",
            params![sort_order, cd_id, author_id],
        )?;
        Ok(affected > 0)
    }

    pub fn swap_track_positions(
        &self,
        cd_id: i64,
        first_track_id: i64,
        second_track_id: i64,
    ) -> Result<bool, rusqlite::Error> {
        if first_track_id == second_track_id {
            return Ok(false);
        }
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let load = |track_id: i64| {
            tx.query_row(
                "SELECT cd_id, disc_number, track_number FROM tracks WHERE id = ?1",
                params![track_id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
        };
        let Some((first_cd, first_disc, first_number)) = load(first_track_id)? else {
            return Ok(false);
        };
        let Some((second_cd, second_disc, second_number)) = load(second_track_id)? else {
            return Ok(false);
        };
        if first_cd != Some(cd_id) || second_cd != Some(cd_id) || first_disc != second_disc {
            return Ok(false);
        }
        tx.execute(
            "UPDATE tracks SET disc_number = ?1, track_number = ?2 WHERE id = ?3",
            params![second_disc, second_number, first_track_id],
        )?;
        tx.execute(
            "UPDATE tracks SET disc_number = ?1, track_number = ?2 WHERE id = ?3",
            params![first_disc, first_number, second_track_id],
        )?;
        tx.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_cd() -> NewCd {
        NewCd {
            jan: None,
            title: "manual CD".to_string(),
            artist: Some("artist".to_string()),
            publisher: None,
            label: None,
            catalog_number: None,
            publish_date: None,
            cover_url: None,
            description: None,
            disc_count: Some(2),
            volume: None,
            tracks: None,
            parent_book_id: None,
            media_type: Some("cd".to_string()),
            series_id: None,
        }
    }

    #[test]
    fn manual_cd_creation_is_atomic_and_keeps_metadata() {
        let db = Db::new(":memory:").expect("database");
        let author = db.create_author("artist", None, None).expect("author").id;
        let tracks = vec![
            (
                NewTrack {
                    disc_number: Some(1),
                    track_number: 1,
                    title: "first".to_string(),
                    duration: Some("01:00".to_string()),
                },
                Some(TrackMetadata {
                    title: Some("first".to_string()),
                    artist: Some("artist".to_string()),
                    ..TrackMetadata::default()
                }),
            ),
            (
                NewTrack {
                    disc_number: Some(2),
                    track_number: 1,
                    title: "second disc".to_string(),
                    duration: None,
                },
                None,
            ),
        ];
        let metadata = CdMetadata {
            year: Some(2026),
            genre: Some("test".to_string()),
            ..CdMetadata::default()
        };

        let cd = db
            .insert_manual_cd(&new_cd(), &[author], &tracks, Some(&metadata), None)
            .expect("manual CD");
        assert_eq!(db.list_tracks_for_cd(cd.id).unwrap().len(), 2);
        assert_eq!(db.get_cd_authors(cd.id).unwrap().len(), 1);
        assert_eq!(db.get_cd_metadata(cd.id).unwrap().unwrap().year, Some(2026));
        assert_eq!(
            db.get_track_metadata(db.list_tracks_for_cd(cd.id).unwrap()[0].id)
                .unwrap()
                .unwrap()
                .artist,
            Some("artist".to_string())
        );

        let failed = db.insert_manual_cd(&new_cd(), &[999], &[], None, None);
        assert!(failed.is_err());
        assert_eq!(db.list_cds().unwrap().len(), 1);
    }

    #[test]
    fn swaps_only_tracks_from_the_same_cd_and_disc() {
        let db = Db::new(":memory:").expect("database");
        let cd = db.insert_cd(&new_cd()).expect("CD");
        let first = db
            .insert_track_for_cd(
                cd.id,
                &NewTrack {
                    disc_number: Some(1),
                    track_number: 1,
                    title: "first".to_string(),
                    duration: None,
                },
            )
            .expect("first track");
        let second = db
            .insert_track_for_cd(
                cd.id,
                &NewTrack {
                    disc_number: Some(1),
                    track_number: 2,
                    title: "second".to_string(),
                    duration: None,
                },
            )
            .expect("second track");
        assert!(db.swap_track_positions(cd.id, first.id, second.id).unwrap());
        let tracks = db.list_tracks_for_cd(cd.id).unwrap();
        assert_eq!(tracks[0].id, second.id);
        assert_eq!(tracks[1].id, first.id);
        assert!(!db.swap_track_positions(cd.id, first.id, 999).unwrap());
    }
}
