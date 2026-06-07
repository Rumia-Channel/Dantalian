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

    pub fn get_track_metadata_with_cd_inheritance(
        &self,
        track_id: i64,
    ) -> Result<Option<TrackMetadata>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let cd_id: Option<i64> = conn
            .query_row(
                "SELECT cd_id FROM tracks WHERE id = ?1",
                params![track_id],
                |row| row.get(0),
            )
            .ok();

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
        let track_row = rows.next()?;
        let mut meta = match track_row {
            Some(r) => TrackMetadata {
                title: r.get(0)?,
                artist: r.get(1)?,
                album: r.get(2)?,
                album_artist: r.get(3)?,
                track_number: r.get(4)?,
                track_total: r.get(5)?,
                disc_number: r.get(6)?,
                disc_total: r.get(7)?,
                year: r.get(8)?,
                genre: r.get(9)?,
                composer: r.get(10)?,
                publisher: r.get(11)?,
                label: r.get(12)?,
                encoder: r.get(13)?,
                comment: r.get(14)?,
                lyrics: r.get(15)?,
                cover_mime: r.get(16)?,
                cover_data: r.get(17)?,
                replay_gain_track_gain_db: r.get(18)?,
                replay_gain_track_peak: r.get(19)?,
                replay_gain_album_gain_db: r.get(20)?,
                replay_gain_album_peak: r.get(21)?,
                file_type: r.get(22)?,
                raw_size_bytes: r.get(23)?,
            },
            None => return Ok(None),
        };

        if let Some(cd_id) = cd_id {
            let mut cstmt = conn.prepare(
                "SELECT title, artist, album_artist, publisher, label
                 FROM cds WHERE id = ?1",
            )?;
            let mut crows = cstmt.query(params![cd_id])?;
            if let Some(cr) = crows.next()? {
                if meta.album.is_none() { meta.album = cr.get(0)?; }
                if meta.artist.is_none() { meta.artist = cr.get(1)?; }
                if meta.album_artist.is_none() { meta.album_artist = cr.get(2)?; }
                if meta.publisher.is_none() { meta.publisher = cr.get(3)?; }
                if meta.label.is_none() { meta.label = cr.get(4)?; }
            }

            let mut mstmt = conn.prepare(
                "SELECT artist, album_artist, year, genre, composer, isrc,
                        cover_mime, cover_data,
                        replay_gain_album_gain_db, replay_gain_album_peak
                 FROM cd_metadata WHERE cd_id = ?1",
            )?;
            let mut mrows = mstmt.query(params![cd_id])?;
            if let Some(mr) = mrows.next()? {
                if meta.artist.is_none() { meta.artist = mr.get(0)?; }
                if meta.album_artist.is_none() { meta.album_artist = mr.get(1)?; }
                if meta.year.is_none() { meta.year = mr.get(2)?; }
                if meta.genre.is_none() { meta.genre = mr.get(3)?; }
                if meta.composer.is_none() { meta.composer = mr.get(4)?; }
                if meta.cover_mime.is_none() { meta.cover_mime = mr.get(5)?; }
                if meta.cover_data.is_none() { meta.cover_data = mr.get(6)?; }
                if meta.replay_gain_album_gain_db.is_none() { meta.replay_gain_album_gain_db = mr.get(7)?; }
                if meta.replay_gain_album_peak.is_none() { meta.replay_gain_album_peak = mr.get(8)?; }
            }
        }

        Ok(Some(meta))
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
