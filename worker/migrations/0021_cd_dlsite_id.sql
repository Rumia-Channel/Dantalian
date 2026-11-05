ALTER TABLE cds ADD COLUMN dlsite_id TEXT;
CREATE UNIQUE INDEX idx_cds_dlsite_id ON cds(dlsite_id) WHERE dlsite_id IS NOT NULL;
