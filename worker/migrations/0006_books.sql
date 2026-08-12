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
    series_id INTEGER REFERENCES series(id) ON DELETE SET NULL,
    series_number INTEGER,
    media_type TEXT NOT NULL DEFAULT 'book'
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_books_isdn ON books(isdn) WHERE isdn IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_books_jan ON books(jan) WHERE jan IS NOT NULL;
