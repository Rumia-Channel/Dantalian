ALTER TABLE audio_jobs ADD COLUMN idempotency_key TEXT;
ALTER TABLE audio_jobs ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE audio_jobs ADD COLUMN lease_until TEXT;
ALTER TABLE audio_jobs ADD COLUMN started_at TEXT;
ALTER TABLE audio_jobs ADD COLUMN finished_at TEXT;
ALTER TABLE audio_jobs ADD COLUMN next_attempt_at TEXT;
ALTER TABLE audio_jobs ADD COLUMN processor_id TEXT;
ALTER TABLE audio_jobs ADD COLUMN lease_token TEXT;
ALTER TABLE audio_jobs ADD COLUMN provider_job_id TEXT;
ALTER TABLE audio_jobs ADD COLUMN output_size_bytes INTEGER;

CREATE UNIQUE INDEX IF NOT EXISTS idx_audio_jobs_owner_idempotency
    ON audio_jobs(owner, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_audio_jobs_owner_active_output
    ON audio_jobs(owner, output_object_key)
    WHERE status IN ('queued', 'running', 'completed');

CREATE INDEX IF NOT EXISTS idx_audio_jobs_claimable
    ON audio_jobs(owner, status, next_attempt_at, lease_until, created_at);
