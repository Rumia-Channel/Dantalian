-- Indexes for the hot read paths: list pages, detail pages, lending counts,
-- playlist playback, and cascading deletes. Additive only; no table or column
-- changes, so existing rows are untouched.
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
