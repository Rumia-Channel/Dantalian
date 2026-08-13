CREATE TABLE IF NOT EXISTS audio_jobs (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    input_object_key TEXT NOT NULL,
    output_object_key TEXT NOT NULL,
    codec TEXT NOT NULL CHECK (codec IN ('opus', 'aac')),
    bitrate_kbps INTEGER NOT NULL CHECK (bitrate_kbps BETWEEN 8 AND 512),
    error_summary TEXT,
    owner TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audio_jobs_owner_status_updated
    ON audio_jobs(owner, status, updated_at);
