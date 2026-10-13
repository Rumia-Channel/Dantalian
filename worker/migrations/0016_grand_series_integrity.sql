DELETE FROM grand_series_items
WHERE (item_type = 'series' AND NOT EXISTS (SELECT 1 FROM series WHERE id = item_id))
   OR (item_type = 'book' AND NOT EXISTS (SELECT 1 FROM books WHERE id = item_id))
   OR (item_type = 'cd' AND NOT EXISTS (SELECT 1 FROM cds WHERE id = item_id));

CREATE TRIGGER IF NOT EXISTS grand_series_delete_series AFTER DELETE ON series
BEGIN
    DELETE FROM grand_series_items WHERE item_type = 'series' AND item_id = OLD.id;
END;
CREATE TRIGGER IF NOT EXISTS grand_series_delete_book AFTER DELETE ON books
BEGIN
    DELETE FROM grand_series_items WHERE item_type = 'book' AND item_id = OLD.id;
END;
CREATE TRIGGER IF NOT EXISTS grand_series_delete_cd AFTER DELETE ON cds
BEGIN
    DELETE FROM grand_series_items WHERE item_type = 'cd' AND item_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS grand_series_validate_insert BEFORE INSERT ON grand_series_items
WHEN (NEW.item_type = 'series' AND NOT EXISTS (SELECT 1 FROM series WHERE id = NEW.item_id))
  OR (NEW.item_type = 'book' AND NOT EXISTS (SELECT 1 FROM books WHERE id = NEW.item_id))
  OR (NEW.item_type = 'cd' AND NOT EXISTS (SELECT 1 FROM cds WHERE id = NEW.item_id))
BEGIN
    SELECT RAISE(ABORT, 'Grand series item not found');
END;
CREATE TRIGGER IF NOT EXISTS grand_series_validate_update BEFORE UPDATE OF item_type, item_id ON grand_series_items
WHEN (NEW.item_type = 'series' AND NOT EXISTS (SELECT 1 FROM series WHERE id = NEW.item_id))
  OR (NEW.item_type = 'book' AND NOT EXISTS (SELECT 1 FROM books WHERE id = NEW.item_id))
  OR (NEW.item_type = 'cd' AND NOT EXISTS (SELECT 1 FROM cds WHERE id = NEW.item_id))
BEGIN
    SELECT RAISE(ABORT, 'Grand series item not found');
END;
