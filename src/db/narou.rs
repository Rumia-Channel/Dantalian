use super::Db;
use crate::narou::{IMPORT_SQL, ImportMapping, MAPPING_SQL, Novel};
use rusqlite::{OptionalExtension, ToSql, params};

impl Db {
    pub fn cached_narou_novel(
        &self,
        source: &str,
        id: i64,
    ) -> rusqlite::Result<Option<crate::narou::CachedNovel>> {
        self.0
            .lock()
            .query_row(crate::narou::CACHE_SQL, params![source, id], |row| {
                Ok(crate::narou::CachedNovel {
                    book_id: row.get(0)?,
                    file_hash: row.get(1)?,
                    remote_version: row.get(2)?,
                })
            })
            .optional()
    }

    pub fn reconcile_narou_selection(&self, source: &str, ids: &[i64]) -> rusqlite::Result<()> {
        let ids = serde_json::to_string(ids).expect("serializing novel IDs");
        self.0
            .lock()
            .execute(crate::narou::RECONCILE_SQL, params![source, ids])?;
        Ok(())
    }

    pub fn import_narou_novel(
        &self,
        source: &str,
        novel: &Novel,
        hash: &str,
        filename: &str,
        version: &str,
    ) -> rusqlite::Result<(i64, bool, Option<String>)> {
        let mut conn = self.0.lock();
        let tx = conn.transaction()?;
        let previous = tx
            .query_row(MAPPING_SQL, params![source, novel.id], |row| {
                Ok(ImportMapping {
                    book_id: row.get(0)?,
                    file_hash: row.get(1)?,
                })
            })
            .optional()?;
        let author =
            if let Some((id, _, _)) = Self::find_author_by_normalized_name(&tx, &novel.author)? {
                tx.query_row("SELECT name FROM authors WHERE id=?1", [id], |row| {
                    row.get::<_, String>(0)
                })?
            } else {
                novel.author.trim().to_owned()
            };
        let description = novel.description();
        let values: [&dyn ToSql; 9] = [
            &source,
            &novel.id,
            &novel.title,
            &novel.sitename,
            &description,
            &author,
            &hash,
            &filename,
            &version,
        ];
        for (sql, count) in IMPORT_SQL {
            tx.execute(sql, &values[..*count])?;
        }
        let id = tx.query_row(MAPPING_SQL, params![source, novel.id], |row| row.get(0))?;
        tx.commit()?;
        Ok((
            id,
            previous.is_none(),
            previous.map(|mapping| mapping.file_hash),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn novel() -> Novel {
        Novel {
            id: 42,
            title: "小説".into(),
            author: "著者".into(),
            tags: vec!["対象".into()],
            sitename: "小説サイト".into(),
            display_url: Some("https://example.com/novel".into()),
            toc_url: None,
            last_update: 1,
            general_lastup: None,
        }
    }

    #[test]
    fn narou_refresh_preserves_identity_state_and_manual_attachments() {
        let db = Db::new(":memory:").unwrap();
        let mut novel = novel();
        let (id, created, _) = db
            .import_narou_novel("https://narou.test", &novel, "one.epub", "小説.epub", "1")
            .unwrap();
        assert!(created);
        db.0.lock()
            .execute(
                "UPDATE books SET reading_status='reading' WHERE id=?1",
                [id],
            )
            .unwrap();
        db.add_book_file(id, "manual.pdf", "手動.pdf").unwrap();
        novel.title = "更新版".into();
        let (updated_id, created, previous) = db
            .import_narou_novel("https://narou.test", &novel, "two.epub", "更新版.epub", "2")
            .unwrap();
        assert_eq!(updated_id, id);
        assert!(!created);
        assert_eq!(previous.as_deref(), Some("one.epub"));
        let book = db.find_by_id(id).unwrap().unwrap();
        assert_eq!(book.title, "更新版");
        assert_eq!(book.reading_status.as_deref(), Some("reading"));
        assert_eq!(
            book.files
                .iter()
                .map(|f| f.file_hash.as_str())
                .collect::<Vec<_>>(),
            ["manual.pdf", "two.epub"]
        );
        assert_eq!(db.get_book_authors(id).unwrap()[0].name, "著者");
        let (other, created, _) = db
            .import_narou_novel(
                "https://another.test",
                &novel,
                "two.epub",
                "更新版.epub",
                "2",
            )
            .unwrap();
        assert!(created);
        assert_ne!(other, id);
        db.delete_book(id).unwrap();
        assert!(
            db.import_narou_novel("https://narou.test", &novel, "two.epub", "更新版.epub", "2")
                .unwrap()
                .1
        );
    }

    #[test]
    fn narou_failed_attachment_rolls_back_new_and_updated_books() {
        let db = Db::new(":memory:").unwrap();
        let mut novel = novel();
        let (id, _, _) = db
            .import_narou_novel("source", &novel, "one.epub", "one.epub", "1")
            .unwrap();
        db.0.lock().execute_batch("CREATE TRIGGER reject_narou_file BEFORE INSERT ON book_files BEGIN SELECT RAISE(ABORT,'storage failure'); END;").unwrap();
        novel.title = "保存されない更新".into();
        assert!(
            db.import_narou_novel("source", &novel, "two.epub", "two.epub", "2")
                .is_err()
        );
        let book = db.find_by_id(id).unwrap().unwrap();
        assert_eq!(book.title, "小説");
        assert_eq!(book.files[0].file_hash, "one.epub");
        novel.id = 43;
        assert!(
            db.import_narou_novel("source", &novel, "two.epub", "two.epub", "2")
                .is_err()
        );
        assert_eq!(
            db.0.lock()
                .query_row("SELECT COUNT(*) FROM books", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn narou_visibility_filters_before_pagination_and_preserves_removed_state() {
        let db = Db::new(":memory:").unwrap();
        db.0.lock()
            .execute("INSERT INTO books(title) VALUES ('通常の本')", [])
            .unwrap();
        let mut novel = novel();
        let (id, _, _) = db
            .import_narou_novel("source", &novel, "one.epub", "one.epub", "version-1")
            .unwrap();
        assert_eq!(
            db.list_books_page(1, None, true).unwrap()[0].title,
            "通常の本"
        );
        assert_eq!(db.list_books_page(1, None, false).unwrap()[0].id, id);
        assert_eq!(
            db.cached_narou_novel("source", novel.id)
                .unwrap()
                .unwrap()
                .remote_version,
            "version-1"
        );
        db.0.lock()
            .execute(
                "UPDATE books SET reading_status='reading' WHERE id=?1",
                [id],
            )
            .unwrap();
        db.reconcile_narou_selection("source", &[]).unwrap();
        assert_eq!(
            db.list_books_page(10, None, false)
                .unwrap()
                .iter()
                .map(|book| book.title.as_str())
                .collect::<Vec<_>>(),
            ["通常の本"]
        );
        assert_eq!(
            db.find_by_id(id)
                .unwrap()
                .unwrap()
                .reading_status
                .as_deref(),
            Some("reading")
        );
        db.reconcile_narou_selection("source", &[novel.id]).unwrap();
        assert_eq!(db.list_books_page(1, None, false).unwrap()[0].id, id);
        novel.last_update += 1;
        let version = serde_json::to_string(&novel).unwrap();
        db.import_narou_novel("source", &novel, "two.epub", "two.epub", &version)
            .unwrap();
        assert_eq!(
            db.cached_narou_novel("source", novel.id)
                .unwrap()
                .unwrap()
                .file_hash,
            "two.epub"
        );
    }
}
