use super::Db;
use crate::db_models::CdMetadata;
use rusqlite::params;

impl Db {
    pub fn upsert_cd_metadata(
        &self,
        cd_id: i64,
        m: &CdMetadata,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let cover_blob: Option<&[u8]> = m.cover_data.as_deref();
        conn.execute(
            r#"
            INSERT INTO cd_metadata (
                cd_id, artist, album, album_artist,
                year, genre, composer, publisher, label, catalog_number, isrc,
                cover_mime, cover_data,
                replay_gain_album_gain_db, replay_gain_album_peak,
                updated_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15, CURRENT_TIMESTAMP)
            ON CONFLICT(cd_id) DO UPDATE SET
                artist=excluded.artist,
                album=excluded.album,
                album_artist=excluded.album_artist,
                year=excluded.year,
                genre=excluded.genre,
                composer=excluded.composer,
                publisher=excluded.publisher,
                label=excluded.label,
                catalog_number=excluded.catalog_number,
                isrc=excluded.isrc,
                cover_mime=excluded.cover_mime,
                cover_data=excluded.cover_data,
                replay_gain_album_gain_db=excluded.replay_gain_album_gain_db,
                replay_gain_album_peak=excluded.replay_gain_album_peak,
                updated_at=CURRENT_TIMESTAMP
            "#,
            params![
                cd_id,
                m.artist,
                m.album,
                m.album_artist,
                m.year,
                m.genre,
                m.composer,
                m.publisher,
                m.label,
                m.catalog_number,
                m.isrc,
                m.cover_mime,
                cover_blob,
                m.replay_gain_album_gain_db,
                m.replay_gain_album_peak,
            ],
        )?;
        Ok(())
    }

    pub fn get_cd_metadata(&self, cd_id: i64) -> Result<Option<CdMetadata>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT artist, album, album_artist, year, genre, composer, publisher, label,
                    catalog_number, isrc, cover_mime, cover_data,
                    replay_gain_album_gain_db, replay_gain_album_peak
             FROM cd_metadata WHERE cd_id = ?1",
        )?;
        let mut rows = stmt.query(params![cd_id])?;
        let Some(row) = rows.next()? else { return Ok(None) };
        Ok(Some(CdMetadata {
            cd_id,
            artist: row.get(0)?,
            album: row.get(1)?,
            album_artist: row.get(2)?,
            year: row.get(3)?,
            genre: row.get(4)?,
            composer: row.get(5)?,
            publisher: row.get(6)?,
            label: row.get(7)?,
            catalog_number: row.get(8)?,
            isrc: row.get(9)?,
            cover_mime: row.get(10)?,
            cover_data: row.get(11)?,
            replay_gain_album_gain_db: row.get(12)?,
            replay_gain_album_peak: row.get(13)?,
        }))
    }

    pub fn delete_cd_metadata(&self, cd_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM cd_metadata WHERE cd_id = ?1",
            params![cd_id],
        )?;
        Ok(())
    }
}
