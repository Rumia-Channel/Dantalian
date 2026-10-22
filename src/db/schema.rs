use super::Db;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use std::sync::Arc;

const SCHEMA_VERSION: u32 = 14;

/// Idempotent indexes for the hot read paths (list/detail pages, lending
/// counts, playlist playback). Executed on every open so existing databases
/// pick them up without a schema version bump.
const READ_INDEXES_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_copies_book_id ON copies(book_id);
CREATE INDEX IF NOT EXISTS idx_lending_history_copy_returned ON lending_history(copy_id, returned_date);
CREATE INDEX IF NOT EXISTS idx_lending_history_borrower ON lending_history(borrower_id);
CREATE INDEX IF NOT EXISTS idx_tracks_book_id ON tracks(book_id);
CREATE INDEX IF NOT EXISTS idx_tracks_cd_id ON tracks(cd_id);
CREATE INDEX IF NOT EXISTS idx_book_authors_author_id ON book_authors(author_id);
CREATE INDEX IF NOT EXISTS idx_cd_authors_author_id ON cd_authors(author_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_track_id ON playlist_tracks(track_id);
CREATE INDEX IF NOT EXISTS idx_books_series_id ON books(series_id);
CREATE INDEX IF NOT EXISTS idx_books_storage_location_id ON books(storage_location_id);
CREATE INDEX IF NOT EXISTS idx_books_label_id ON books(label_id);
CREATE INDEX IF NOT EXISTS idx_cds_parent_book_id ON cds(parent_book_id);
CREATE INDEX IF NOT EXISTS idx_cds_series_id ON cds(series_id);
CREATE INDEX IF NOT EXISTS idx_grand_series_items_item ON grand_series_items(item_type, item_id);
"#;

const GRAND_SERIES_INTEGRITY_SQL: &str = r#"
DELETE FROM grand_series_items
WHERE (item_type = 'series' AND NOT EXISTS (SELECT 1 FROM series WHERE id = item_id))
   OR (item_type = 'book' AND NOT EXISTS (SELECT 1 FROM books WHERE id = item_id))
   OR (item_type = 'cd' AND NOT EXISTS (SELECT 1 FROM cds WHERE id = item_id));
CREATE TRIGGER IF NOT EXISTS grand_series_delete_series AFTER DELETE ON series
BEGIN
    DELETE FROM grand_series_items WHERE item_type = 'series' AND item_id = OLD.id;
END;
CREATE TRIGGER IF NOT EXISTS grand_series_delete_book AFTER DELETE ON books
BEGIN
    DELETE FROM grand_series_items WHERE item_type = 'book' AND item_id = OLD.id;
END;
CREATE TRIGGER IF NOT EXISTS grand_series_delete_cd AFTER DELETE ON cds
BEGIN
    DELETE FROM grand_series_items WHERE item_type = 'cd' AND item_id = OLD.id;
END;
CREATE TRIGGER IF NOT EXISTS grand_series_validate_insert BEFORE INSERT ON grand_series_items
WHEN (NEW.item_type = 'series' AND NOT EXISTS (SELECT 1 FROM series WHERE id = NEW.item_id))
  OR (NEW.item_type = 'book' AND NOT EXISTS (SELECT 1 FROM books WHERE id = NEW.item_id))
  OR (NEW.item_type = 'cd' AND NOT EXISTS (SELECT 1 FROM cds WHERE id = NEW.item_id))
BEGIN
    SELECT RAISE(ABORT, 'Grand series item not found');
END;
CREATE TRIGGER IF NOT EXISTS grand_series_validate_update BEFORE UPDATE OF item_type, item_id ON grand_series_items
WHEN (NEW.item_type = 'series' AND NOT EXISTS (SELECT 1 FROM series WHERE id = NEW.item_id))
  OR (NEW.item_type = 'book' AND NOT EXISTS (SELECT 1 FROM books WHERE id = NEW.item_id))
  OR (NEW.item_type = 'cd' AND NOT EXISTS (SELECT 1 FROM cds WHERE id = NEW.item_id))
BEGIN
    SELECT RAISE(ABORT, 'Grand series item not found');
END;
"#;

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS series (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS grand_series (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS grand_series_items (
    grand_series_id INTEGER NOT NULL REFERENCES grand_series(id) ON DELETE CASCADE,
    item_type TEXT NOT NULL CHECK(item_type IN ('series', 'book', 'cd')),
    item_id INTEGER NOT NULL,
    PRIMARY KEY (grand_series_id, item_type, item_id)
);

CREATE TABLE IF NOT EXISTS authors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ndl_id TEXT UNIQUE,
    name TEXT NOT NULL,
    transcription TEXT
);

CREATE TABLE IF NOT EXISTS storage_locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER REFERENCES storage_locations(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    isbn TEXT UNIQUE,
    isdn TEXT,
    jan TEXT,
    title TEXT NOT NULL,
    publisher TEXT,
    publish_date TEXT,
    cover_url TEXT,
    description TEXT,
    title_transcription TEXT,
    series_title TEXT,
    series_title_transcription TEXT,
    alternative TEXT,
    alternative_transcription TEXT,
    volume TEXT,
    volume_transcription TEXT,
    price TEXT,
    extent TEXT,
    jpno TEXT,
    ndl_url TEXT,
    series_id INTEGER REFERENCES series(id) ON DELETE SET NULL,
    series_number INTEGER,
    isdn_region TEXT,
    isdn_class TEXT,
    isdn_type TEXT,
    isdn_rating_gender TEXT,
    isdn_rating_age TEXT,
    isdn_genre_code TEXT,
    isdn_genre_name TEXT,
    isdn_genre_user TEXT,
    isdn_c_code TEXT,
    isdn_author TEXT,
    isdn_shape TEXT,
    isdn_contents TEXT,
    isdn_barcode2 TEXT,
    isdn_sample_image_url TEXT,
    isdn_useroption TEXT,
    isdn_external_links TEXT,
    media_type TEXT NOT NULL DEFAULT 'book',
    catalog_number TEXT,
    artist TEXT,
    label TEXT,
    disc_count INTEGER,
    epub_file_hash TEXT,
    epub_file_name TEXT,
    reading_status TEXT DEFAULT 'unread',
    storage_location_id INTEGER REFERENCES storage_locations(id) ON DELETE SET NULL,
    label_id INTEGER REFERENCES labels(id) ON DELETE SET NULL,
    created_at TEXT,
    updated_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_books_isdn ON books(isdn) WHERE isdn IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_books_jan ON books(jan) WHERE jan IS NOT NULL;

CREATE TABLE IF NOT EXISTS book_authors (
    book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (book_id, author_id)
);

CREATE TABLE IF NOT EXISTS copies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    copy_type TEXT NOT NULL DEFAULT 'physical' CHECK(copy_type IN ('physical', 'ebook')),
    location TEXT,
    notes TEXT
);

CREATE TABLE IF NOT EXISTS borrowers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    notes TEXT
);

CREATE TABLE IF NOT EXISTS lending_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    copy_id INTEGER NOT NULL REFERENCES copies(id) ON DELETE CASCADE,
    borrower_id INTEGER NOT NULL REFERENCES borrowers(id),
    lent_date TEXT NOT NULL,
    due_date TEXT,
    returned_date TEXT,
    notes TEXT
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    jan TEXT UNIQUE,
    title TEXT NOT NULL,
    artist TEXT,
    publisher TEXT,
    label TEXT,
    catalog_number TEXT,
    publish_date TEXT,
    cover_url TEXT,
    description TEXT,
    disc_count INTEGER,
    volume TEXT,
    created_at TEXT,
    updated_at TEXT,
    parent_book_id INTEGER REFERENCES books(id) ON DELETE SET NULL,
    media_type TEXT NOT NULL DEFAULT 'cd',
    series_id INTEGER REFERENCES series(id) ON DELETE SET NULL,
    series_number INTEGER
);

CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id INTEGER REFERENCES books(id) ON DELETE CASCADE,
    cd_id INTEGER REFERENCES cds(id) ON DELETE CASCADE,
    disc_number INTEGER NOT NULL DEFAULT 1,
    track_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    duration TEXT,
    file_hash TEXT,
    file_name TEXT
);

CREATE TABLE IF NOT EXISTS playlists (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    description TEXT,
    cover_cd_id INTEGER REFERENCES cds(id) ON DELETE SET NULL,
    created_at  TEXT,
    updated_at  TEXT
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, track_id)
);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_order
    ON playlist_tracks(playlist_id, position);

CREATE TABLE IF NOT EXISTS cd_authors (
    cd_id INTEGER NOT NULL REFERENCES cds(id) ON DELETE CASCADE,
    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (cd_id, author_id)
);

CREATE TABLE IF NOT EXISTS track_metadata (
    track_id           INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    title              TEXT,
    artist             TEXT,
    album              TEXT,
    album_artist       TEXT,
    track_number       INTEGER,
    track_total        INTEGER,
    disc_number        INTEGER,
    disc_total         INTEGER,
    year               INTEGER,
    genre              TEXT,
    composer           TEXT,
    publisher          TEXT,
    label              TEXT,
    encoder            TEXT,
    comment            TEXT,
    lyrics             TEXT,
    cover_mime         TEXT,
    cover_data         BLOB,
    replay_gain_track_gain_db REAL,
    replay_gain_track_peak    REAL,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak    REAL,
    encoder_vendor     TEXT,
    file_type          TEXT,
    raw_size_bytes     INTEGER,
    custom_json        TEXT NOT NULL DEFAULT '{}',
    created_at         DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at         DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_tm_artist ON track_metadata(artist);
CREATE INDEX IF NOT EXISTS idx_tm_album  ON track_metadata(album);
CREATE INDEX IF NOT EXISTS idx_tm_year   ON track_metadata(year);

CREATE TABLE IF NOT EXISTS cd_metadata (
    cd_id              INTEGER PRIMARY KEY REFERENCES cds(id) ON DELETE CASCADE,
    year               INTEGER,
    genre              TEXT,
    composer           TEXT,
    isrc               TEXT,
    cover_mime         TEXT,
    cover_data         BLOB,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak    REAL,
    updated_at         DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_cdm_year   ON cd_metadata(year);

CREATE TABLE IF NOT EXISTS track_authors (
    track_id   INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    author_id  INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, author_id)
);
CREATE INDEX IF NOT EXISTS idx_ta_track ON track_authors(track_id);
CREATE INDEX IF NOT EXISTS idx_ta_author ON track_authors(author_id);
"#;

const MIGRATE_V1_TO_V2_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS track_metadata (
    track_id           INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    title              TEXT,
    artist             TEXT,
    album              TEXT,
    album_artist       TEXT,
    track_number       INTEGER,
    track_total        INTEGER,
    disc_number        INTEGER,
    disc_total         INTEGER,
    year               INTEGER,
    genre              TEXT,
    composer           TEXT,
    encoder            TEXT,
    comment            TEXT,
    lyrics             TEXT,
    cover_mime         TEXT,
    cover_data         BLOB,
    replay_gain_track_gain_db REAL,
    replay_gain_track_peak    REAL,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak    REAL,
    encoder_vendor     TEXT,
    file_type          TEXT,
    raw_size_bytes     INTEGER,
    custom_json        TEXT NOT NULL DEFAULT '{}',
    created_at         DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at         DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_tm_artist ON track_metadata(artist);
CREATE INDEX IF NOT EXISTS idx_tm_album  ON track_metadata(album);
CREATE INDEX IF NOT EXISTS idx_tm_year   ON track_metadata(year);
"#;

const MIGRATE_V2_TO_V3_SQL: &str = r#"
ALTER TABLE track_metadata ADD COLUMN publisher TEXT;
ALTER TABLE track_metadata ADD COLUMN label TEXT;
"#;

const MIGRATE_V3_TO_V4_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS cd_metadata (
    cd_id              INTEGER PRIMARY KEY REFERENCES cds(id) ON DELETE CASCADE,
    artist             TEXT,
    album              TEXT,
    album_artist       TEXT,
    year               INTEGER,
    genre              TEXT,
    composer           TEXT,
    publisher          TEXT,
    label              TEXT,
    catalog_number     TEXT,
    isrc               TEXT,
    cover_mime         TEXT,
    cover_data         BLOB,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak    REAL,
    updated_at         DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_cdm_artist ON cd_metadata(artist);
CREATE INDEX IF NOT EXISTS idx_cdm_album  ON cd_metadata(album);
CREATE INDEX IF NOT EXISTS idx_cdm_year   ON cd_metadata(year);

INSERT OR IGNORE INTO cd_metadata (
    cd_id, artist, album, album_artist, year, genre, composer,
    publisher, label, replay_gain_album_gain_db, replay_gain_album_peak
)
SELECT
    t.cd_id,
    MAX(tm.artist),
    MAX(tm.album),
    MAX(tm.album_artist),
    MAX(tm.year),
    MAX(tm.genre),
    MAX(tm.composer),
    MAX(tm.publisher),
    MAX(tm.label),
    MAX(tm.replay_gain_album_gain_db),
    MAX(tm.replay_gain_album_peak)
FROM tracks t
JOIN track_metadata tm ON tm.track_id = t.id
WHERE t.cd_id IS NOT NULL
GROUP BY t.cd_id;
"#;

const MIGRATE_V4_TO_V5_SQL: &str = r#"
UPDATE cds
SET title         = COALESCE(cds.title,         (SELECT MAX(album)          FROM cd_metadata WHERE cd_metadata.cd_id = cds.id)),
    publisher     = COALESCE(cds.publisher,     (SELECT MAX(publisher)      FROM cd_metadata WHERE cd_metadata.cd_id = cds.id)),
    label         = COALESCE(cds.label,         (SELECT MAX(label)          FROM cd_metadata WHERE cd_metadata.cd_id = cds.id)),
    catalog_number= COALESCE(cds.catalog_number,(SELECT MAX(catalog_number) FROM cd_metadata WHERE cd_metadata.cd_id = cds.id))
WHERE EXISTS (SELECT 1 FROM cd_metadata WHERE cd_metadata.cd_id = cds.id);

CREATE TABLE IF NOT EXISTS cd_metadata_v5 (
    cd_id              INTEGER PRIMARY KEY REFERENCES cds(id) ON DELETE CASCADE,
    artist             TEXT,
    album_artist       TEXT,
    year               INTEGER,
    genre              TEXT,
    composer           TEXT,
    isrc               TEXT,
    cover_mime         TEXT,
    cover_data         BLOB,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak    REAL,
    updated_at         DATETIME DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO cd_metadata_v5 (cd_id, artist, album_artist, year, genre, composer, isrc,
                            cover_mime, cover_data,
                            replay_gain_album_gain_db, replay_gain_album_peak, updated_at)
SELECT cd_id, artist, album_artist, year, genre, composer, isrc,
       cover_mime, cover_data,
       replay_gain_album_gain_db, replay_gain_album_peak, updated_at
FROM cd_metadata;

DROP TABLE cd_metadata;
ALTER TABLE cd_metadata_v5 RENAME TO cd_metadata;
CREATE INDEX IF NOT EXISTS idx_cdm_artist ON cd_metadata(artist);
CREATE INDEX IF NOT EXISTS idx_cdm_year   ON cd_metadata(year);
"#;

const MIGRATE_V5_TO_V6_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS cd_metadata_v6 (
    cd_id              INTEGER PRIMARY KEY REFERENCES cds(id) ON DELETE CASCADE,
    year               INTEGER,
    genre              TEXT,
    composer           TEXT,
    isrc               TEXT,
    cover_mime         TEXT,
    cover_data         BLOB,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak    REAL,
    updated_at         DATETIME DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO cd_metadata_v6 (cd_id, year, genre, composer, isrc,
                            cover_mime, cover_data,
                            replay_gain_album_gain_db, replay_gain_album_peak, updated_at)
SELECT cd_id, year, genre, composer, isrc,
       cover_mime, cover_data,
       replay_gain_album_gain_db, replay_gain_album_peak, updated_at
FROM cd_metadata;

DROP TABLE cd_metadata;
ALTER TABLE cd_metadata_v6 RENAME TO cd_metadata;
CREATE INDEX IF NOT EXISTS idx_cdm_year ON cd_metadata(year);

CREATE TABLE IF NOT EXISTS track_authors (
    track_id   INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    author_id  INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, author_id)
);
CREATE INDEX IF NOT EXISTS idx_ta_track ON track_authors(track_id);
CREATE INDEX IF NOT EXISTS idx_ta_author ON track_authors(author_id);
"#;

const MIGRATE_V6_TO_V7_SQL: &str = r#"
ALTER TABLE books ADD COLUMN epub_file_hash TEXT;
ALTER TABLE books ADD COLUMN epub_file_name TEXT;
"#;

const MIGRATE_V7_TO_V8_SQL: &str = r#"
ALTER TABLE books ADD COLUMN reading_status TEXT DEFAULT 'unread';

CREATE TABLE IF NOT EXISTS storage_locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER REFERENCES storage_locations(id) ON DELETE SET NULL
);

ALTER TABLE books ADD COLUMN storage_location_id INTEGER REFERENCES storage_locations(id) ON DELETE SET NULL;
"#;

const MIGRATE_V8_TO_V9_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

ALTER TABLE books ADD COLUMN label_id INTEGER REFERENCES labels(id) ON DELETE SET NULL;
"#;

const MIGRATE_V10_TO_V11_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS playlists (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    description TEXT,
    cover_cd_id INTEGER REFERENCES cds(id) ON DELETE SET NULL,
    created_at  TEXT,
    updated_at  TEXT
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, track_id)
);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_order
    ON playlist_tracks(playlist_id, position);
"#;

fn ensure_track_metadata_columns(conn: &Connection) -> Result<(), rusqlite::Error> {
    let columns: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(track_metadata)")?;
        let rows = stmt.query_map([], |row| row.get(1))?;
        rows.collect::<Result<_, _>>()?
    };

    if !columns.iter().any(|column| column == "publisher") {
        conn.execute("ALTER TABLE track_metadata ADD COLUMN publisher TEXT", [])?;
    }
    if !columns.iter().any(|column| column == "label") {
        conn.execute("ALTER TABLE track_metadata ADD COLUMN label TEXT", [])?;
    }
    Ok(())
}

/// Audiobooks/CDs gained a user-series volume (`series_number`), matching books.
/// Runs on every open so existing databases pick up the column regardless of the
/// schema version they started from (the ALTER is skipped when it already
/// exists, which keeps legacy-version test fixtures working).
fn ensure_cd_series_number_column(conn: &Connection) -> Result<(), rusqlite::Error> {
    let columns: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(cds)")?;
        let rows = stmt.query_map([], |row| row.get(1))?;
        rows.collect::<Result<_, _>>()?
    };
    if !columns.iter().any(|column| column == "series_number") {
        conn.execute("ALTER TABLE cds ADD COLUMN series_number INTEGER", [])?;
    }
    Ok(())
}

fn migrate_publish_dates(conn: &Connection) -> Result<(), rusqlite::Error> {
    for table in ["books", "cds"] {
        let rows: Vec<(i64, String)> = {
            let mut stmt = conn.prepare(&format!(
                "SELECT id, publish_date FROM {} WHERE publish_date IS NOT NULL AND publish_date <> ''",
                table
            ))?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };

        for (id, raw) in rows {
            match crate::external::normalize_publish_date(Some(&raw)) {
                Some(normalized) if normalized != raw => {
                    conn.execute(
                        &format!(
                            "UPDATE {} SET publish_date = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                            table
                        ),
                        params![normalized, id],
                    )?;
                }
                Some(_) => {}
                None => {
                    tracing::warn!(table, id, value = %raw, "Could not normalize publish_date during migration")
                }
            }
        }
    }
    Ok(())
}

impl Db {
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let current_version: u32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

        if current_version == 0 {
            conn.execute_batch(SCHEMA_SQL)?;
            ensure_track_metadata_columns(&conn)?;
            conn.execute_batch(GRAND_SERIES_INTEGRITY_SQL)?;
            conn.execute_batch(&format!("PRAGMA user_version = {};", SCHEMA_VERSION))?;
            tracing::info!(version = SCHEMA_VERSION, "Database schema initialized");
        } else if current_version < SCHEMA_VERSION {
            tracing::info!(
                from = current_version,
                to = SCHEMA_VERSION,
                "Migrating database schema"
            );
            if current_version < 2 {
                conn.execute_batch(MIGRATE_V1_TO_V2_SQL)?;
            }
            if current_version < 3 {
                conn.execute_batch(MIGRATE_V2_TO_V3_SQL)?;
            }
            if current_version < 4 {
                conn.execute_batch(MIGRATE_V3_TO_V4_SQL)?;
            }
            if current_version < 5 {
                conn.execute_batch(MIGRATE_V4_TO_V5_SQL)?;
            }
            if current_version < 6 {
                conn.execute_batch(MIGRATE_V5_TO_V6_SQL)?;
            }
            if current_version < 7 {
                conn.execute_batch(MIGRATE_V6_TO_V7_SQL)?;
            }
            if current_version < 8 {
                conn.execute_batch(MIGRATE_V7_TO_V8_SQL)?;
            }
            if current_version < 9 {
                conn.execute_batch(MIGRATE_V8_TO_V9_SQL)?;
            }
            if current_version < 11 {
                conn.execute_batch(MIGRATE_V10_TO_V11_SQL)?;
            }
            if current_version < 12 {
                migrate_publish_dates(&conn)?;
            }
            if current_version < 13 {
                conn.execute_batch(GRAND_SERIES_INTEGRITY_SQL)?;
            }
            ensure_track_metadata_columns(&conn)?;
            conn.execute_batch(&format!("PRAGMA user_version = {};", SCHEMA_VERSION))?;
            tracing::info!(version = SCHEMA_VERSION, "Database schema migrated");
        } else if current_version > SCHEMA_VERSION {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
                Some(format!(
                    "Database schema {current_version} is newer than supported schema {SCHEMA_VERSION}; refusing to modify it"
                )),
            ));
        }

        conn.execute_batch(READ_INDEXES_SQL)?;
        ensure_cd_series_number_column(&conn)?;

        Ok(Self(Arc::new(Mutex::new(conn))))
    }
}

#[cfg(test)]
mod tests {
    use super::Db;
    use rusqlite::params;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn migrates_legacy_publish_date_formats() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dantalian-date-migration-{}-{}.db",
            std::process::id(),
            suffix
        ));
        let path_str = path.to_string_lossy().to_string();

        let db = Db::new(&path_str).expect("create database");
        {
            let conn = db.0.lock();
            conn.execute(
                "INSERT INTO books (title, publish_date) VALUES (?1, ?2)",
                params!["legacy book", "2007.1"],
            )
            .expect("insert legacy book");
            conn.execute(
                "INSERT INTO cds (title, publish_date) VALUES (?1, ?2)",
                params!["legacy cd", "２０２６年２月１３日"],
            )
            .expect("insert legacy cd");
            conn.execute_batch("PRAGMA user_version = 11;")
                .expect("set legacy schema version");
        }
        drop(db);

        let migrated = Db::new(&path_str).expect("migrate database");
        let conn = migrated.0.lock();
        let book_date: String = conn
            .query_row(
                "SELECT publish_date FROM books WHERE title = 'legacy book'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated book date");
        let cd_date: String = conn
            .query_row(
                "SELECT publish_date FROM cds WHERE title = 'legacy cd'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated cd date");

        assert_eq!(book_date, "2007-01-NN");
        assert_eq!(cd_date, "2026-02-13");

        drop(conn);
        drop(migrated);
        let _ = std::fs::remove_file(path);
    }

    fn temporary_path(label: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "dantalian-{label}-{}-{suffix}.db",
            std::process::id()
        ))
    }

    #[test]
    fn newer_schema_is_rejected_without_erasing_data() {
        let path = temporary_path("newer-schema");
        let db = Db::new(path.to_str().unwrap()).unwrap();
        let future_version = super::SCHEMA_VERSION + 1;
        {
            let conn = db.0.lock();
            conn.execute("INSERT INTO books(title) VALUES ('Keep this book')", [])
                .unwrap();
            conn.pragma_update(None, "user_version", future_version)
                .unwrap();
        }
        drop(db);
        assert!(Db::new(path.to_str().unwrap()).is_err());
        let conn = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(
            conn.query_row("SELECT title FROM books", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "Keep this book"
        );
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            future_version
        );
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn migration_prunes_dead_members_and_deletion_cleans_polymorphic_links() {
        let path = temporary_path("grand-series");
        let db = Db::new(path.to_str().unwrap()).unwrap();
        {
            let conn = db.0.lock();
            conn.execute_batch(
                "DROP TRIGGER grand_series_validate_insert;
                 INSERT INTO grand_series(name) VALUES ('Group');
                 INSERT INTO books(title) VALUES ('Book');
                 INSERT INTO series(name) VALUES ('Series');
                 INSERT INTO cds(title) VALUES ('Album');
                 INSERT INTO grand_series_items VALUES (1,'book',1),(1,'series',1),(1,'cd',1),(1,'book',999);
                 PRAGMA user_version=12;"
            ).unwrap();
        }
        drop(db);
        let migrated = Db::new(path.to_str().unwrap()).unwrap();
        assert_eq!(migrated.list_grand_series().unwrap()[0].items.len(), 3);
        {
            let conn = migrated.0.lock();
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM grand_series_items", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                3
            );
            assert!(
                conn.execute("INSERT INTO grand_series_items VALUES (1,'cd',999)", [])
                    .is_err()
            );
            conn.execute("DELETE FROM books WHERE id=1", []).unwrap();
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM grand_series_items", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            conn.execute("DELETE FROM cds WHERE id=1", []).unwrap();
            conn.execute("DELETE FROM series WHERE id=1", []).unwrap();
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM grand_series_items", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        assert!(migrated.list_grand_series().unwrap()[0].items.is_empty());
        drop(migrated);
        std::fs::remove_file(path).unwrap();
    }
}
