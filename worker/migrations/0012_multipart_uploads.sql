CREATE TABLE IF NOT EXISTS multipart_uploads (
    id TEXT PRIMARY KEY,
    provider_upload_id TEXT NOT NULL,
    object_key TEXT NOT NULL UNIQUE,
    object_kind TEXT NOT NULL CHECK (object_kind = 'epub'),
    expected_size TEXT NOT NULL,
    content_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'uploading', 'complete', 'aborted', 'failed')),
    owner TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_multipart_uploads_status_created
    ON multipart_uploads(status, created_at);
