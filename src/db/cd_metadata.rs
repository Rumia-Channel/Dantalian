use super::Db;
use crate::db_models::CdMetadata;
use rusqlite::{Connection, params};

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn get_cd_metadata_conn(
    conn: &Connection,
    cd_id: i64,
) -> Result<Option<CdMetadata>, rusqlite::Error> {
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

    /// 音声ファイルから得た CD メタデータを、項目単位で既存値と統合する。
    ///
    /// `upsert_cd_metadata` は編集画面からの明示的な保存にも使われるため、
    /// `None` を「その項目をクリアする」という意味で扱う。音声タグの抽出結果は
    /// ファイル形式によって一部の項目が空になるため、アップロード時だけは空の
    /// 項目で既存の値を上書きしない。
    pub fn merge_cd_metadata_from_audio(
        &self,
        cd_id: i64,
        incoming: &CdMetadata,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let existing = get_cd_metadata_conn(&conn, cd_id)?.unwrap_or_default();
        let merged = CdMetadata {
            cd_id,
            year: incoming.year.or(existing.year),
            genre: non_empty_text(incoming.genre.clone()).or(existing.genre),
            composer: non_empty_text(incoming.composer.clone()).or(existing.composer),
            isrc: non_empty_text(incoming.isrc.clone()).or(existing.isrc),
            cover_mime: non_empty_text(incoming.cover_mime.clone()).or(existing.cover_mime),
            cover_data: incoming
                .cover_data
                .clone()
                .filter(|data| !data.is_empty())
                .or(existing.cover_data),
            replay_gain_album_gain_db: incoming
                .replay_gain_album_gain_db
                .or(existing.replay_gain_album_gain_db),
            replay_gain_album_peak: incoming
                .replay_gain_album_peak
                .or(existing.replay_gain_album_peak),
        };
        upsert_cd_metadata_conn(&conn, cd_id, &merged)
    }

    pub fn get_cd_metadata(&self, cd_id: i64) -> Result<Option<CdMetadata>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        get_cd_metadata_conn(&conn, cd_id)
    }

    pub fn delete_cd_metadata(&self, cd_id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM cd_metadata WHERE cd_id = ?1", params![cd_id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_models::NewCd;

    fn new_cd() -> NewCd {
        NewCd {
            jan: None,
            title: "test CD".to_string(),
            artist: None,
            publisher: None,
            label: None,
            catalog_number: None,
            publish_date: None,
            cover_url: None,
            description: None,
            disc_count: None,
            volume: None,
            tracks: None,
            parent_book_id: None,
            media_type: Some("audiobook".to_string()),
            series_id: None,
        }
    }

    #[test]
    fn audio_metadata_merge_keeps_existing_fields_independently() {
        let db = Db::new(":memory:").expect("database");
        let cd = db.insert_cd(&new_cd()).expect("CD");

        db.upsert_cd_metadata(
            cd.id,
            &CdMetadata {
                cd_id: cd.id,
                year: Some(2024),
                genre: Some("existing genre".to_string()),
                composer: None,
                isrc: Some("existing-isrc".to_string()),
                ..CdMetadata::default()
            },
        )
        .expect("existing metadata");

        db.merge_cd_metadata_from_audio(
            cd.id,
            &CdMetadata {
                cd_id: cd.id,
                year: None,
                genre: Some("incoming genre".to_string()),
                composer: Some("incoming composer".to_string()),
                isrc: None,
                ..CdMetadata::default()
            },
        )
        .expect("merge metadata");

        let merged = db.get_cd_metadata(cd.id).expect("read metadata").unwrap();
        assert_eq!(merged.year, Some(2024));
        assert_eq!(merged.genre.as_deref(), Some("incoming genre"));
        assert_eq!(merged.composer.as_deref(), Some("incoming composer"));
        assert_eq!(merged.isrc.as_deref(), Some("existing-isrc"));
    }
}
