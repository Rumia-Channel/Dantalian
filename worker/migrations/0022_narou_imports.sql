CREATE TABLE IF NOT EXISTS narou_imports (
    source_url TEXT NOT NULL,
    novel_id INTEGER NOT NULL,
    book_id INTEGER NOT NULL UNIQUE REFERENCES books(id) ON DELETE CASCADE,
    file_hash TEXT NOT NULL,
    remote_version TEXT NOT NULL DEFAULT '',
    active INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (source_url, novel_id)
);
