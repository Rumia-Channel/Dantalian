CREATE TABLE IF NOT EXISTS object_uploads (
    object_key TEXT PRIMARY KEY,
    object_kind TEXT NOT NULL CHECK(object_kind IN ('cover', 'epub', 'audio')),
    entity_id INTEGER NOT NULL,
    content_type TEXT NOT NULL,
    extension TEXT NOT NULL,
    expected_size INTEGER NOT NULL,
    original_name TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'complete')),
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_object_uploads_entity
    ON object_uploads(object_kind, entity_id);
