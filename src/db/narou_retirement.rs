use std::{fs, io, path::Path};

use rusqlite::{Connection, OpenFlags};

fn has_legacy_cache(conn: &Connection) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='narou_imports')",
        [],
        |row| row.get(0),
    )
}

/// Remove only files owned exclusively by the retired narou mirror before its DB migration.
/// A manual attachment sharing the same content keeps the file alive.
pub fn remove_retired_narou_epubs(db_path: &str, epubs_dir: &str) -> io::Result<()> {
    if !Path::new(db_path).exists() {
        return Ok(());
    }
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(io::Error::other)?;
    if !has_legacy_cache(&conn).map_err(io::Error::other)? {
        return Ok(());
    }
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(io::Error::other)?;
    if version != 18 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retired narou cache has an unsupported schema version",
        ));
    }
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT n.file_hash FROM narou_imports n
             WHERE n.file_hash <> '' AND NOT EXISTS (
                 SELECT 1 FROM book_files f WHERE f.file_hash=n.file_hash
                 AND NOT EXISTS (SELECT 1 FROM narou_imports owned
                     WHERE owned.book_id=f.book_id AND owned.file_hash=f.file_hash)
             )",
        )
        .map_err(io::Error::other)?;
    let hashes = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(io::Error::other)?;
    for hash in hashes {
        let hash = hash.map_err(io::Error::other)?;
        if Path::new(&hash).file_name().and_then(|name| name.to_str()) != Some(hash.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid retired narou attachment filename",
            ));
        }
        match fs::remove_file(Path::new(epubs_dir).join(&hash)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    if !has_legacy_cache(conn)? {
        return Ok(());
    }
    conn.execute_batch(
        "BEGIN;
         DELETE FROM settings WHERE key IN ('narou.token','narou.cf_access_client_id','narou.cf_access_client_secret');
         DELETE FROM book_files WHERE EXISTS (
             SELECT 1 FROM narou_imports n
             WHERE n.book_id=book_files.book_id AND n.file_hash=book_files.file_hash
         );
         DELETE FROM books WHERE id IN (SELECT book_id FROM narou_imports)
             AND NOT EXISTS (SELECT 1 FROM book_files f WHERE f.book_id=books.id)
             AND NOT EXISTS (SELECT 1 FROM copies c WHERE c.book_id=books.id)
             AND NOT EXISTS (SELECT 1 FROM tracks t WHERE t.book_id=books.id)
             AND NOT EXISTS (SELECT 1 FROM grand_series_items g WHERE g.item_type='book' AND g.item_id=books.id)
             AND series_id IS NULL AND storage_location_id IS NULL AND label_id IS NULL
             AND COALESCE(reading_status,'unread')='unread';
         DROP TABLE narou_imports;
         COMMIT;",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn retires_mirror_rows_without_deleting_manual_attachments_or_reading_state() {
        let db = Db::new(":memory:").unwrap();
        let conn = db.0.lock();
        conn.execute_batch(
            "CREATE TABLE narou_imports (book_id INTEGER, file_hash TEXT);
             INSERT INTO books(id,title,reading_status) VALUES
                 (1,'cached','unread'),(2,'manual attachment','unread'),
                 (3,'ordinary','unread'),(4,'reading','reading');
             INSERT INTO book_files(book_id,file_hash,file_name) VALUES
                 (1,'one.epub','one.epub'),(2,'two.epub','two.epub'),
                 (2,'manual.epub','manual.epub'),(3,'ordinary.epub','ordinary.epub'),
                 (4,'four.epub','four.epub');
             INSERT INTO narou_imports VALUES (1,'one.epub'),(2,'two.epub'),(4,'four.epub');",
        )
        .unwrap();
        migrate(&conn).unwrap();
        let books: Vec<i64> = conn
            .prepare("SELECT id FROM books ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(books, [2, 3, 4]);
        let files: Vec<String> = conn
            .prepare("SELECT file_hash FROM book_files ORDER BY file_hash")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(files, ["manual.epub", "ordinary.epub"]);
        assert!(!has_legacy_cache(&conn).unwrap());
    }

    #[test]
    fn deletes_cached_bytes_but_preserves_content_shared_with_manual_books() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "dantalian-narou-retirement-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        let db = Db::new(path.to_str().unwrap()).unwrap();
        {
            let conn = db.0.lock();
            conn.execute_batch(
                "CREATE TABLE narou_imports (book_id INTEGER, file_hash TEXT);
                 INSERT INTO books(id,title) VALUES (1,'cached'),(2,'shared cache'),(3,'manual');
                 INSERT INTO book_files(book_id,file_hash,file_name) VALUES
                     (1,'old.epub','old.epub'),(2,'shared.epub','shared.epub'),
                     (3,'shared.epub','shared.epub'),(3,'manual.epub','manual.epub');
                 INSERT INTO narou_imports VALUES (1,'old.epub'),(2,'shared.epub');",
            )
            .unwrap();
            conn.pragma_update(None, "user_version", 18).unwrap();
        }
        fs::write(dir.join("old.epub"), b"cached").unwrap();
        fs::write(dir.join("shared.epub"), b"shared manual bytes").unwrap();
        fs::write(dir.join("manual.epub"), b"manual bytes").unwrap();
        remove_retired_narou_epubs(path.to_str().unwrap(), dir.to_str().unwrap()).unwrap();
        assert!(!dir.join("old.epub").exists());
        assert_eq!(
            fs::read(dir.join("shared.epub")).unwrap(),
            b"shared manual bytes"
        );
        assert_eq!(fs::read(dir.join("manual.epub")).unwrap(), b"manual bytes");
        drop(db);
        fs::remove_dir_all(dir).unwrap();
    }
}
