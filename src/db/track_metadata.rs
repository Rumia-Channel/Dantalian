use super::Db;
use crate::external::audio_meta::TrackMetadata;
use rusqlite::params;

impl Db {
    pub fn upsert_track_metadata(
        &self,
        track_id: i64,
        m: &TrackMetadata,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let cover_blob: Option<&[u8]> = m.cover_data.as_deref();
        conn.execute(
            r#"
            INSERT INTO track_metadata (
                track_id, title, artist, album, album_artist,
                track_number, track_total, disc_number, disc_total,
                year, genre, composer, publisher, label, encoder, comment, lyrics,
                cover_mime, cover_data,
                replay_gain_track_gain_db, replay_gain_track_peak,
                replay_gain_album_gain_db, replay_gain_album_peak,
                file_type, raw_size_bytes,
                updated_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25, CURRENT_TIMESTAMP)
            ON CONFLICT(track_id) DO UPDATE SET
                title=excluded.title,
                artist=excluded.artist,
                album=excluded.album,
                album_artist=excluded.album_artist,
                track_number=excluded.track_number,
                track_total=excluded.track_total,
                disc_number=excluded.disc_number,
                disc_total=excluded.disc_total,
                year=excluded.year,
                genre=excluded.genre,
                composer=excluded.composer,
                publisher=excluded.publisher,
                label=excluded.label,
                encoder=excluded.encoder,
                comment=excluded.comment,
                lyrics=excluded.lyrics,
                cover_mime=excluded.cover_mime,
                cover_data=excluded.cover_data,
                replay_gain_track_gain_db=excluded.replay_gain_track_gain_db,
                replay_gain_track_peak=excluded.replay_gain_track_peak,
                replay_gain_album_gain_db=excluded.replay_gain_album_gain_db,
                replay_gain_album_peak=excluded.replay_gain_album_peak,
                file_type=excluded.file_type,
                raw_size_bytes=excluded.raw_size_bytes,
                updated_at=CURRENT_TIMESTAMP
            "#,
            params![
                track_id,
                m.title,
                m.artist,
                m.album,
                m.album_artist,
                m.track_number,
                m.track_total,
                m.disc_number,
                m.disc_total,
                m.year,
                m.genre,
                m.composer,
                m.publisher,
                m.label,
                m.encoder,
                m.comment,
                m.lyrics,
                m.cover_mime,
                cover_blob,
                m.replay_gain_track_gain_db,
                m.replay_gain_track_peak,
                m.replay_gain_album_gain_db,
                m.replay_gain_album_peak,
                m.file_type,
                m.raw_size_bytes,
            ],
        )?;
        Ok(())
    }

    pub fn get_track_metadata(&self, track_id: i64) -> Result<Option<TrackMetadata>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT title, artist, album, album_artist, track_number, track_total,
                    disc_number, disc_total, year, genre, composer, publisher, label,
                    encoder, comment, lyrics, cover_mime, cover_data,
                    replay_gain_track_gain_db, replay_gain_track_peak,
                    replay_gain_album_gain_db, replay_gain_album_peak,
                    file_type, raw_size_bytes
             FROM track_metadata WHERE track_id = ?1",
        )?;
        let mut rows = stmt.query(params![track_id])?;
        let Some(row) = rows.next()? else { return Ok(None) };
        Ok(Some(TrackMetadata {
            title: row.get(0)?,
            artist: row.get(1)?,
            album: row.get(2)?,
            album_artist: row.get(3)?,
            track_number: row.get(4)?,
            track_total: row.get(5)?,
            disc_number: row.get(6)?,
            disc_total: row.get(7)?,
            year: row.get(8)?,
            genre: row.get(9)?,
            composer: row.get(10)?,
            publisher: row.get(11)?,
            label: row.get(12)?,
            encoder: row.get(13)?,
            comment: row.get(14)?,
            lyrics: row.get(15)?,
            cover_mime: row.get(16)?,
            cover_data: row.get(17)?,
            replay_gain_track_gain_db: row.get(18)?,
            replay_gain_track_peak: row.get(19)?,
            replay_gain_album_gain_db: row.get(20)?,
            replay_gain_album_peak: row.get(21)?,
            file_type: row.get(22)?,
            raw_size_bytes: row.get(23)?,
        }))
    }

    pub fn delete_track_metadata(&self, track_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM track_metadata WHERE track_id = ?1",
            params![track_id],
        )?;
        Ok(())
    }
}
