use super::Db;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

const SCHEMA_VERSION: u32 = 11;

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
    series_id INTEGER REFERENCES series(id) ON DELETE SET NULL
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

const DROP_ALL_SQL: &str = r#"
DROP TABLE IF EXISTS playlist_tracks;
DROP TABLE IF EXISTS playlists;
DROP TABLE IF EXISTS cd_metadata;
DROP TABLE IF EXISTS track_metadata;
DROP TABLE IF EXISTS cd_authors;
DROP TABLE IF EXISTS tracks;
DROP TABLE IF EXISTS cds;
DROP TABLE IF EXISTS track_metadata;
DROP TABLE IF EXISTS cd_authors;
DROP TABLE IF EXISTS tracks;
DROP TABLE IF EXISTS cds;
DROP TABLE IF EXISTS lending_history;
DROP TABLE IF EXISTS borrowers;
DROP TABLE IF EXISTS copies;
DROP TABLE IF EXISTS book_authors;
DROP TABLE IF EXISTS books;
DROP TABLE IF EXISTS labels;
DROP TABLE IF EXISTS storage_locations;
DROP TABLE IF EXISTS authors;
DROP TABLE IF EXISTS grand_series_items;
DROP TABLE IF EXISTS grand_series;
DROP TABLE IF EXISTS series;
DROP TABLE IF EXISTS settings;
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

impl Db {
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let current_version: u32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

        if current_version == 0 {
            conn.execute_batch(SCHEMA_SQL)?;
            ensure_track_metadata_columns(&conn)?;
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
            ensure_track_metadata_columns(&conn)?;
            conn.execute_batch(&format!("PRAGMA user_version = {};", SCHEMA_VERSION))?;
            tracing::info!(version = SCHEMA_VERSION, "Database schema migrated");
        } else if current_version > SCHEMA_VERSION {
            tracing::warn!(
                current = current_version,
                target = SCHEMA_VERSION,
                "Database schema newer than expected; dropping all tables and recreating"
            );
            conn.execute_batch(DROP_ALL_SQL)?;
            conn.execute_batch(SCHEMA_SQL)?;
            ensure_track_metadata_columns(&conn)?;
            conn.execute_batch(&format!("PRAGMA user_version = {};", SCHEMA_VERSION))?;
        }

        Ok(Self(Arc::new(Mutex::new(conn))))
    }
}
