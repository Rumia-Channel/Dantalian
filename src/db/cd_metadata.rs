use super::Db;
use crate::db_models::CdMetadata;
use rusqlite::{Connection, params};

pub(crate) fn upsert_cd_metadata_conn(
    conn: &Connection,
    cd_id: i64,
    m: &CdMetadata,
) -> Result<(), rusqlite::Error> {
    let cover_blob: Option<&[u8]> = m.cover_data.as_deref();
    conn.execute(
        r#"
        INSERT INTO cd_metadata (
            cd_id, year, genre, composer, isrc,
            cover_mime, cover_data,
            replay_gain_album_gain_db, replay_gain_album_peak,
            updated_at
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9, CURRENT_TIMESTAMP)
        ON CONFLICT(cd_id) DO UPDATE SET
            year=excluded.year,
            genre=excluded.genre,
            composer=excluded.composer,
            isrc=excluded.isrc,
            cover_mime=excluded.cover_mime,
            cover_data=excluded.cover_data,
            replay_gain_album_gain_db=excluded.replay_gain_album_gain_db,
            replay_gain_album_peak=excluded.replay_gain_album_peak,
            updated_at=CURRENT_TIMESTAMP
        "#,
        params![
            cd_id,
            m.year,
            m.genre,
            m.composer,
            m.isrc,
            m.cover_mime,
            cover_blob,
            m.replay_gain_album_gain_db,
            m.replay_gain_album_peak,
        ],
    )?;
    Ok(())
}

impl Db {
    pub fn upsert_cd_metadata(&self, cd_id: i64, m: &CdMetadata) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        upsert_cd_metadata_conn(&conn, cd_id, m)
    }

    pub fn get_cd_metadata(&self, cd_id: i64) -> Result<Option<CdMetadata>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT year, genre, composer, isrc,
                    cover_mime, cover_data,
                    replay_gain_album_gain_db, replay_gain_album_peak
             FROM cd_metadata WHERE cd_id = ?1",
        )?;
        let mut rows = stmt.query(params![cd_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(CdMetadata {
            cd_id,
            year: row.get(0)?,
            genre: row.get(1)?,
            composer: row.get(2)?,
            isrc: row.get(3)?,
            cover_mime: row.get(4)?,
            cover_data: row.get(5)?,
            replay_gain_album_gain_db: row.get(6)?,
            replay_gain_album_peak: row.get(7)?,
        }))
    }

    pub fn delete_cd_metadata(&self, cd_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM cd_metadata WHERE cd_id = ?1", params![cd_id])?;
        Ok(())
    }
}
