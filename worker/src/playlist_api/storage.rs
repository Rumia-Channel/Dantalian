use crate::d1_read::ReadDb;
use serde::Deserialize;
use std::collections::HashMap;
use worker::{D1Database, D1PreparedStatement, D1Type, Result};

const PLAYLIST_SELECT: &str = "SELECT p.id,p.name,p.description,p.cover_cd_id,c.cover_url,p.created_at,p.updated_at FROM playlists p LEFT JOIN cds c ON c.id=p.cover_cd_id";

const TRACK_SELECT: &str = "SELECT pt.playlist_id, pt.position,
         json_object('id',t.id,'book_id',COALESCE(t.book_id,0),'cd_id',t.cd_id,
                     'disc_number',t.disc_number,'track_number',t.track_number,
                     'title',t.title,'duration',t.duration,'file_hash',t.file_hash,'file_name',t.file_name) AS track,
         json_object('id',c.id,'jan',c.jan,'title',c.title,'artist',c.artist,
                     'publisher',c.publisher,'label',c.label,'catalog_number',c.catalog_number,
                     'publish_date',c.publish_date,'cover_url',c.cover_url,'description',c.description,
                     'disc_count',c.disc_count,'volume',c.volume,'created_at',c.created_at,
                     'updated_at',c.updated_at,'parent_book_id',c.parent_book_id,
                     'media_type',c.media_type,'series_id',c.series_id) AS cd
         FROM playlist_tracks pt JOIN tracks t ON t.id=pt.track_id JOIN cds c ON c.id=t.cd_id";

#[derive(Deserialize)]
struct PlaylistTrackRow {
    playlist_id: i64,
    position: i64,
    track: String,
    cd: String,
}

fn with_tracks(mut playlist: serde_json::Value) -> Result<serde_json::Value> {
    let Some(serde_json::Value::String(tracks)) = playlist
        .as_object_mut()
        .and_then(|row| row.remove("tracks_json"))
    else {
        return Err(worker::Error::from("Missing playlist tracks"));
    };
    playlist["tracks"] = serde_json::from_str(&tracks)?;
    Ok(playlist)
}

pub async fn load(db: &D1Database, id: i64) -> Result<Option<serde_json::Value>> {
    db.prepare(format!(
        "SELECT p.*, (SELECT json_group_array(json_object(
             'position',position,'track',json(track),'cd',json(cd)
         )) FROM ({TRACK_SELECT} WHERE pt.playlist_id=p.id ORDER BY pt.position,pt.rowid)) AS tracks_json
         FROM ({PLAYLIST_SELECT}) p WHERE p.id=?"
    ))
        .bind_refs(&D1Type::Integer(id as i32))?
        .first::<serde_json::Value>(None)
        .await?
        .map(with_tracks)
        .transpose()
}

pub async fn list(db: &ReadDb<'_>) -> Result<Vec<serde_json::Value>> {
    let results = db
        .batch(vec![
            db.prepare(format!("{PLAYLIST_SELECT} ORDER BY p.id DESC")),
            db.prepare(format!(
                "{TRACK_SELECT} ORDER BY pt.playlist_id, pt.position, pt.rowid"
            )),
        ])
        .await?;
    let mut playlists = results[0].results::<serde_json::Value>()?;
    let track_rows = results[1].results::<PlaylistTrackRow>()?;
    let mut grouped: HashMap<i64, Vec<serde_json::Value>> = HashMap::with_capacity(playlists.len());
    for row in track_rows {
        let track: serde_json::Value = serde_json::from_str(&row.track)?;
        let cd: serde_json::Value = serde_json::from_str(&row.cd)?;
        grouped
            .entry(row.playlist_id)
            .or_default()
            .push(serde_json::json!({"position": row.position, "track": track, "cd": cd}));
    }
    for playlist in &mut playlists {
        let id = playlist.get("id").and_then(serde_json::Value::as_i64);
        playlist["tracks"] =
            serde_json::Value::Array(id.and_then(|id| grouped.remove(&id)).unwrap_or_default());
    }
    Ok(playlists)
}

pub async fn exists(db: &D1Database, id: i64) -> Result<bool> {
    Ok(db
        .prepare("SELECT id FROM playlists WHERE id=?")
        .bind_refs(&D1Type::Integer(id as i32))?
        .first::<serde_json::Value>(None)
        .await?
        .is_some())
}

pub async fn valid_cover(db: &D1Database, id: i64) -> Result<bool> {
    Ok(db
        .prepare("SELECT id FROM cds WHERE id=?")
        .bind_refs(&D1Type::Integer(id as i32))?
        .first::<serde_json::Value>(None)
        .await?
        .is_some())
}

pub async fn valid_tracks(db: &D1Database, ids: &[i64]) -> Result<bool> {
    if ids.is_empty() {
        return Ok(true);
    }
    #[derive(Deserialize)]
    struct CountRow {
        count: usize,
    }
    let json = serde_json::to_string(ids)?;
    let row = db
        .prepare(
            "SELECT COUNT(*) AS count FROM json_each(?) requested
         JOIN tracks t ON t.id=requested.value JOIN cds c ON c.id=t.cd_id
         WHERE t.file_hash IS NOT NULL AND t.file_hash<>''",
        )
        .bind_refs(&D1Type::Text(&json))?
        .first::<CountRow>(None)
        .await?;
    Ok(row.is_some_and(|row| row.count == ids.len()))
}

fn append_statement(db: &D1Database, id: Option<i64>, ids: &[i64]) -> Result<D1PreparedStatement> {
    let json = serde_json::to_string(ids)?;
    // Materialize the playlist id and old tail before inserts change last_insert_rowid/MAX.
    // Invalidated audio becomes a NOT NULL violation, rolling back the entire batch.
    db.prepare(
        "WITH playlist AS MATERIALIZED (SELECT COALESCE(?1,last_insert_rowid()) AS id),
         tail AS MATERIALIZED (
             SELECT COALESCE(MAX(position)+1,0) AS position FROM playlist_tracks
             WHERE playlist_id=(SELECT id FROM playlist)
         ), additions AS MATERIALIZED (
             SELECT requested.key, requested.value AS track_id,
                    t.file_hash, c.id AS cd_id
             FROM json_each(?2) requested LEFT JOIN tracks t ON t.id=requested.value
             LEFT JOIN cds c ON c.id=t.cd_id
             WHERE NOT EXISTS(SELECT 1 FROM playlist_tracks pt
                              WHERE pt.playlist_id=(SELECT id FROM playlist) AND pt.track_id=requested.value)
         )
         INSERT INTO playlist_tracks(playlist_id,track_id,position)
         SELECT (SELECT id FROM playlist),
                CASE WHEN cd_id IS NOT NULL AND file_hash IS NOT NULL AND file_hash<>'' THEN track_id ELSE NULL END,
                (SELECT position FROM tail)+ROW_NUMBER() OVER (ORDER BY CAST(key AS INTEGER))-1
         FROM additions"
    ).bind_refs([
        &id.map(|id| D1Type::Integer(id as i32)).unwrap_or(D1Type::Null),
        &D1Type::Text(&json),
    ])
}

fn touch_statement(db: &D1Database, id: i64) -> Result<D1PreparedStatement> {
    // Must immediately follow the track insert/delete in the same batch.
    db.prepare("UPDATE playlists SET updated_at=CURRENT_TIMESTAMP WHERE id=? AND changes()>0")
        .bind_refs(&D1Type::Integer(id as i32))
}

fn compact_statement(db: &D1Database, id: i64) -> Result<D1PreparedStatement> {
    db.prepare(
        "WITH ordered AS MATERIALIZED (
             SELECT track_id,ROW_NUMBER() OVER (ORDER BY position,rowid)-1 AS position
             FROM playlist_tracks WHERE playlist_id=?1
         ) UPDATE playlist_tracks SET position=(
             SELECT position FROM ordered WHERE ordered.track_id=playlist_tracks.track_id
         ) WHERE playlist_id=?1 AND changes()>0 AND position IS NOT (
             SELECT position FROM ordered WHERE ordered.track_id=playlist_tracks.track_id
         )",
    )
    .bind_refs(&D1Type::Integer(id as i32))
}

pub async fn create(
    db: &D1Database,
    name: &str,
    description: Option<&str>,
    cover: Option<i64>,
    tracks: &[i64],
) -> Result<i64> {
    let insert = db
        .prepare(
            "INSERT INTO playlists(name,description,cover_cd_id,created_at,updated_at)
         VALUES(?,?,?,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP) RETURNING id",
        )
        .bind_refs([
            &D1Type::Text(name),
            &description.map(D1Type::Text).unwrap_or(D1Type::Null),
            &cover
                .map(|id| D1Type::Integer(id as i32))
                .unwrap_or(D1Type::Null),
        ])?;
    let results = db
        .batch(vec![insert, append_statement(db, None, tracks)?])
        .await?;
    #[derive(Deserialize)]
    struct IdRow {
        id: i64,
    }
    results[0]
        .results::<IdRow>()?
        .into_iter()
        .next()
        .map(|row| row.id)
        .ok_or_else(|| worker::Error::from("playlist insert returned no row"))
}

pub async fn update(
    db: &D1Database,
    id: i64,
    name: Option<&str>,
    description: Option<Option<&str>>,
    cover: Option<Option<i64>>,
    tracks: Option<&[i64]>,
) -> Result<()> {
    let update = db
        .prepare(
            "UPDATE playlists SET name=COALESCE(?1,name),
         description=CASE WHEN ?2 THEN ?3 ELSE description END,
         cover_cd_id=CASE WHEN ?4 THEN ?5 ELSE cover_cd_id END,
         updated_at=CURRENT_TIMESTAMP WHERE id=?6",
        )
        .bind_refs([
            &name.map(D1Type::Text).unwrap_or(D1Type::Null),
            &D1Type::Integer(i32::from(description.is_some())),
            &description
                .flatten()
                .map(D1Type::Text)
                .unwrap_or(D1Type::Null),
            &D1Type::Integer(i32::from(cover.is_some())),
            &cover
                .flatten()
                .map(|id| D1Type::Integer(id as i32))
                .unwrap_or(D1Type::Null),
            &D1Type::Integer(id as i32),
        ])?;
    let mut statements = vec![update];
    if let Some(tracks) = tracks {
        statements.push(
            db.prepare("DELETE FROM playlist_tracks WHERE playlist_id=?")
                .bind_refs(&D1Type::Integer(id as i32))?,
        );
        statements.push(append_statement(db, Some(id), tracks)?);
    }
    db.batch(statements).await?;
    Ok(())
}

pub async fn append(db: &D1Database, id: i64, tracks: &[i64]) -> Result<()> {
    db.batch(vec![
        append_statement(db, Some(id), tracks)?,
        touch_statement(db, id)?,
    ])
    .await?;
    Ok(())
}

pub async fn remove(db: &D1Database, id: i64, tracks: &[i64]) -> Result<u64> {
    let json = serde_json::to_string(tracks)?;
    let delete = db.prepare("DELETE FROM playlist_tracks WHERE playlist_id=? AND track_id IN (SELECT value FROM json_each(?))")
        .bind_refs([&D1Type::Integer(id as i32),&D1Type::Text(&json)])?;
    let result = db
        .batch(vec![
            delete,
            touch_statement(db, id)?,
            compact_statement(db, id)?,
        ])
        .await?;
    Ok(result[0]
        .meta()?
        .and_then(|meta| meta.changes)
        .unwrap_or_default() as u64)
}

pub async fn delete(db: &D1Database, id: i64) -> Result<bool> {
    let result = db
        .prepare("DELETE FROM playlists WHERE id=?")
        .bind_refs(&D1Type::Integer(id as i32))?
        .run()
        .await?;
    Ok(result
        .meta()?
        .and_then(|meta| meta.changes)
        .unwrap_or_default()
        > 0)
}
