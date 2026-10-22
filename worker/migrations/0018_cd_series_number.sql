-- Audiobooks/CDs gained a user-series volume, mirroring books.series_number.
ALTER TABLE cds ADD COLUMN series_number INTEGER;
