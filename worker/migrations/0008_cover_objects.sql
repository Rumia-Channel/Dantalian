CREATE TABLE IF NOT EXISTS cover_objects (
    object_key TEXT PRIMARY KEY,
    book_id INTEGER REFERENCES books(id) ON DELETE SET NULL,
    content_type TEXT NOT NULL,
    extension TEXT NOT NULL,
    expected_size INTEGER NOT NULL,
    content_sha3_256 TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'complete')),
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cover_objects_book_id ON cover_objects(book_id);
