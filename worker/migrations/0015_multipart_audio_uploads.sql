-- Presigned multipart uploads were EPUB-only. Audio tracks use the same
-- browser-to-Wasabi contract, so the object kind check must accept 'audio'
-- too. SQLite cannot relax a CHECK constraint in place, so the table is
-- rebuilt with the wider constraint and the existing rows are copied over.
CREATE TABLE multipart_uploads_v2 (
    id TEXT PRIMARY KEY,
    provider_upload_id TEXT NOT NULL,
    object_key TEXT NOT NULL UNIQUE,
    object_kind TEXT NOT NULL CHECK (object_kind IN ('epub', 'audio')),
    expected_size TEXT NOT NULL,
    content_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'uploading', 'complete', 'finalized', 'aborted', 'failed')),
    owner TEXT NOT NULL,
    created_at TEXT NOT NULL
);

INSERT INTO multipart_uploads_v2
    (id, provider_upload_id, object_key, object_kind, expected_size, content_type, status, owner, created_at)
SELECT id, provider_upload_id, object_key, object_kind, expected_size, content_type, status, owner, created_at
FROM multipart_uploads;

DROP TABLE multipart_uploads;
ALTER TABLE multipart_uploads_v2 RENAME TO multipart_uploads;

CREATE INDEX IF NOT EXISTS idx_multipart_uploads_status_created
    ON multipart_uploads(status, created_at);
