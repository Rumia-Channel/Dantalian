use super::Db;
use crate::domain::media::BookFile;
use rusqlite::{Connection, OptionalExtension, params};

impl Db {
    pub(super) fn book_files_on(
        conn: &Connection,
        book_id: i64,
    ) -> rusqlite::Result<Vec<BookFile>> {
        conn.prepare("SELECT id, book_id, file_hash, file_name, created_at FROM book_files WHERE book_id = ?1 ORDER BY id")?
            .query_map([book_id], |row| Ok(BookFile {
                id: row.get(0)?, book_id: row.get(1)?, file_hash: row.get(2)?,
                file_name: row.get(3)?, created_at: row.get(4)?,
            }))?.collect()
    }

    pub fn add_book_file(&self, book_id: i64, hash: &str, name: &str) -> rusqlite::Result<()> {
        let mut conn = self.0.lock();
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO book_files(book_id, file_hash, file_name, created_at) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP) ON CONFLICT(book_id, file_hash) DO UPDATE SET file_name = excluded.file_name", params![book_id, hash, name])?;
        tx.execute(
            "UPDATE books SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [book_id],
        )?;
        tx.commit()
    }

    pub fn remove_book_file(&self, book_id: i64, file_id: i64) -> rusqlite::Result<Option<String>> {
        let mut conn = self.0.lock();
        let tx = conn.transaction()?;
        let hash = tx
            .query_row(
                "SELECT file_hash FROM book_files WHERE book_id = ?1 AND id = ?2",
                params![book_id, file_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if hash.is_some() {
            tx.execute(
                "DELETE FROM book_files WHERE book_id = ?1 AND id = ?2",
                params![book_id, file_id],
            )?;
            tx.execute(
                "UPDATE books SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                [book_id],
            )?;
        }
        tx.commit()?;
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachments_append_deduplicate_and_enforce_ownership() {
        let db = Db::new(":memory:").unwrap();
        db.0.lock()
            .execute_batch("INSERT INTO books(id,title) VALUES (1,'First'),(2,'Second');")
            .unwrap();
        db.add_book_file(1, "shared.epub", "first.epub").unwrap();
        db.add_book_file(1, "other.pdf", "other.pdf").unwrap();
        db.add_book_file(1, "shared.epub", "renamed.epub").unwrap();
        db.add_book_file(2, "shared.epub", "second.epub").unwrap();
        let files = db.find_by_id(1).unwrap().unwrap().files;
        assert_eq!(
            files
                .iter()
                .map(|file| file.file_name.as_str())
                .collect::<Vec<_>>(),
            ["renamed.epub", "other.pdf"]
        );
        assert_eq!(db.remove_book_file(2, files[0].id).unwrap(), None);
        assert_eq!(
            db.remove_book_file(1, files[0].id).unwrap().as_deref(),
            Some("shared.epub")
        );
        assert_eq!(
            db.find_by_id(1).unwrap().unwrap().files[0].file_hash,
            "other.pdf"
        );
        assert_eq!(db.count_books_by_epub_hash("shared.epub").unwrap(), 1);
        db.delete_book(2).unwrap();
        assert_eq!(db.count_books_by_epub_hash("shared.epub").unwrap(), 0);
        assert!(db.add_book_file(999, "missing.zip", "missing.zip").is_err());
    }
}
