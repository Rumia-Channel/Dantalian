CREATE TABLE book_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    file_hash TEXT NOT NULL,
    file_name TEXT NOT NULL,
    created_at TEXT,
    UNIQUE(book_id, file_hash)
);
CREATE INDEX idx_book_files_hash ON book_files(file_hash);
INSERT INTO book_files(book_id, file_hash, file_name, created_at)
SELECT id, epub_file_hash, COALESCE(epub_file_name, epub_file_hash), updated_at
FROM books WHERE epub_file_hash IS NOT NULL AND epub_file_hash <> '';
ALTER TABLE books DROP COLUMN epub_file_hash;
ALTER TABLE books DROP COLUMN epub_file_name;
