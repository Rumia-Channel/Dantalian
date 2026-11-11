-- Cached objects must be removed before this retirement; ordinary attachments stay owned locally.
DELETE FROM book_files WHERE EXISTS (
    SELECT 1 FROM narou_imports n
    WHERE n.book_id=book_files.book_id AND n.file_hash=book_files.file_hash
);

DELETE FROM object_uploads WHERE object_kind='epub'
    AND EXISTS (
        SELECT 1 FROM narou_imports n
        WHERE n.book_id=object_uploads.entity_id
            AND substr(object_uploads.object_key,-length(n.file_hash)-1)='/' || n.file_hash
    )
    AND NOT EXISTS (
        SELECT 1 FROM book_files f
        WHERE substr(object_uploads.object_key,-length(f.file_hash)-1)='/' || f.file_hash
    );

DELETE FROM books WHERE id IN (SELECT book_id FROM narou_imports)
    AND NOT EXISTS (SELECT 1 FROM book_files f WHERE f.book_id=books.id)
    AND NOT EXISTS (SELECT 1 FROM copies c WHERE c.book_id=books.id)
    AND NOT EXISTS (SELECT 1 FROM tracks t WHERE t.book_id=books.id)
    AND NOT EXISTS (SELECT 1 FROM grand_series_items g WHERE g.item_type='book' AND g.item_id=books.id)
    AND series_id IS NULL AND storage_location_id IS NULL AND label_id IS NULL
    AND COALESCE(reading_status,'unread')='unread';

DELETE FROM settings WHERE key IN ('narou.token','narou.cf_access_client_id','narou.cf_access_client_secret');

DROP TABLE narou_imports;
