use super::Db;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

const SCHEMA_VERSION: u32 = 1;

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

CREATE TABLE IF NOT EXISTS cd_authors (
    cd_id INTEGER NOT NULL REFERENCES cds(id) ON DELETE CASCADE,
    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (cd_id, author_id)
);
"#;

const DROP_ALL_SQL: &str = r#"
DROP TABLE IF EXISTS cd_authors;
DROP TABLE IF EXISTS tracks;
DROP TABLE IF EXISTS cds;
DROP TABLE IF EXISTS lending_history;
DROP TABLE IF EXISTS borrowers;
DROP TABLE IF EXISTS copies;
DROP TABLE IF EXISTS book_authors;
DROP TABLE IF EXISTS books;
DROP TABLE IF EXISTS authors;
DROP TABLE IF EXISTS grand_series_items;
DROP TABLE IF EXISTS grand_series;
DROP TABLE IF EXISTS series;
DROP TABLE IF EXISTS settings;
"#;

impl Db {
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let current_version: u32 =
            conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

        if current_version != SCHEMA_VERSION {
            if current_version != 0 {
                tracing::warn!(
                    current = current_version,
                    target = SCHEMA_VERSION,
                    "Database schema mismatch (pre-1.0); dropping all tables and recreating"
                );
                conn.execute_batch(DROP_ALL_SQL)?;
            }
            conn.execute_batch(SCHEMA_SQL)?;
            conn.execute_batch(&format!("PRAGMA user_version = {};", SCHEMA_VERSION))?;
            tracing::info!(version = SCHEMA_VERSION, "Database schema initialized");
        }

        Ok(Self(Arc::new(Mutex::new(conn))))
    }
}
