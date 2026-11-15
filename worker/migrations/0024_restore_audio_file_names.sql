-- Recover names lost by presigned audio finalize without changing object keys.
-- Only replace the exact generated name for the track's current audio object.
WITH recovered_names AS (
    SELECT t.id, MIN(TRIM(o.original_name)) AS original_name
    FROM tracks AS t
    JOIN object_uploads AS o ON o.entity_id = t.id
        AND o.object_kind = 'audio'
        AND o.status = 'complete'
        AND t.file_name = t.file_hash || '.' || o.extension
        AND (o.object_key = 'audio/' || t.file_name
            OR SUBSTR(o.object_key, -LENGTH('/audio/' || t.file_name)) = '/audio/' || t.file_name)
    WHERE TRIM(o.original_name) <> ''
    GROUP BY t.id
    HAVING COUNT(DISTINCT TRIM(o.original_name)) = 1
)
UPDATE tracks
SET file_name = (SELECT original_name FROM recovered_names WHERE id = tracks.id)
WHERE id IN (SELECT id FROM recovered_names);
