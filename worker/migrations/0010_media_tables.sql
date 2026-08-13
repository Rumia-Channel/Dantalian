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

CREATE TABLE IF NOT EXISTS copies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    copy_type TEXT NOT NULL DEFAULT 'physical' CHECK(copy_type IN ('physical', 'ebook')),
    location TEXT,
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
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    cover_cd_id INTEGER REFERENCES cds(id) ON DELETE SET NULL,
    created_at TEXT,
    updated_at TEXT
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
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
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    title TEXT,
    artist TEXT,
    album TEXT,
    album_artist TEXT,
    track_number INTEGER,
    track_total INTEGER,
    disc_number INTEGER,
    disc_total INTEGER,
    year INTEGER,
    genre TEXT,
    composer TEXT,
    publisher TEXT,
    label TEXT,
    encoder TEXT,
    comment TEXT,
    lyrics TEXT,
    cover_mime TEXT,
    cover_data BLOB,
    replay_gain_track_gain_db REAL,
    replay_gain_track_peak REAL,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak REAL,
    encoder_vendor TEXT,
    file_type TEXT,
    raw_size_bytes INTEGER,
    custom_json TEXT NOT NULL DEFAULT '{}',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_tm_artist ON track_metadata(artist);
CREATE INDEX IF NOT EXISTS idx_tm_album ON track_metadata(album);
CREATE INDEX IF NOT EXISTS idx_tm_year ON track_metadata(year);

CREATE TABLE IF NOT EXISTS cd_metadata (
    cd_id INTEGER PRIMARY KEY REFERENCES cds(id) ON DELETE CASCADE,
    year INTEGER,
    genre TEXT,
    composer TEXT,
    isrc TEXT,
    cover_mime TEXT,
    cover_data BLOB,
    replay_gain_album_gain_db REAL,
    replay_gain_album_peak REAL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_cdm_year ON cd_metadata(year);

CREATE TABLE IF NOT EXISTS track_authors (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, author_id)
);
CREATE INDEX IF NOT EXISTS idx_ta_track ON track_authors(track_id);
CREATE INDEX IF NOT EXISTS idx_ta_author ON track_authors(author_id);
