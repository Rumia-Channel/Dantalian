ALTER TABLE books ADD COLUMN dlsite_id TEXT;
CREATE UNIQUE INDEX idx_books_dlsite_id ON books(dlsite_id) WHERE dlsite_id IS NOT NULL;
