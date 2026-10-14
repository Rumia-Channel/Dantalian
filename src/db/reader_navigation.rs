use super::Db;
use crate::application::reader_navigation::{NAVIGATION_QUERY, NavigationRow};
use rusqlite::{OptionalExtension, params};

impl Db {
    pub fn reader_navigation(
        &self,
        id: i64,
        content_hash: &str,
    ) -> Result<Option<NavigationRow>, rusqlite::Error> {
        self.0
            .lock()
            .query_row(NAVIGATION_QUERY, params![id, content_hash], |row| {
                Ok(NavigationRow {
                    id: row.get(0)?,
                    media_type: row.get(1)?,
                    epub_file_hash: row.get(2)?,
                    epub_file_name: row.get(3)?,
                    series_id: row.get(4)?,
                    series_name: row.get(5)?,
                    previous_id: row.get(6)?,
                    previous_title: row.get(7)?,
                    previous_hash: row.get(8)?,
                    previous_file_name: row.get(9)?,
                    next_id: row.get(10)?,
                    next_title: row.get(11)?,
                    next_hash: row.get(12)?,
                    next_file_name: row.get(13)?,
                })
            })
            .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::reader_navigation::{NavigationQuery, resolve};

    #[test]
    fn registered_order_handles_gaps_duplicates_nulls_and_unreadable_neighbors() {
        let db = Db::new(":memory:").unwrap();
        {
            let connection = db.0.lock();
            connection
                .execute("INSERT INTO series (id,name) VALUES (1,'Series')", [])
                .unwrap();
            for (id, number, media, hash, name) in [
                (1, Some(1), "book", Some("h1"), Some("one.epub")),
                (2, Some(3), "book", Some("h2"), Some("two.epub")),
                (3, Some(3), "book", None, None),
                (4, None, "book", Some("h4"), Some("four.pdf")),
                (5, Some(2), "audiobook", Some("h5"), Some("five.epub")),
                (6, Some(4), "cd", Some("h6"), Some("six.epub")),
            ] {
                connection.execute("INSERT INTO books (id,title,series_id,series_number,media_type,epub_file_hash,epub_file_name) VALUES (?1,'Book',1,?2,?3,?4,?5)", params![id, number, media, hash, name]).unwrap();
            }
            connection.execute("INSERT INTO books (id,title,epub_file_hash,epub_file_name) VALUES (7,'Standalone','h7','seven.epub')", []).unwrap();
        }
        let first = db.reader_navigation(1, "h1").unwrap().unwrap();
        assert_eq!((first.previous_id, first.next_id), (None, Some(2)));
        let second = db.reader_navigation(2, "h2").unwrap().unwrap();
        assert_eq!((second.previous_id, second.next_id), (Some(1), Some(3)));
        let query = NavigationQuery {
            book_key: Some("book:2".to_owned()),
            content_hash: Some("h2".to_owned()),
            return_to: None,
        }
        .validate()
        .unwrap();
        let next = resolve(Some(second), query).unwrap().next.unwrap();
        assert_eq!(next.status, "missing-file");
        assert!(next.href.is_none());
        let last = db.reader_navigation(4, "h4").unwrap().unwrap();
        assert_eq!((last.previous_id, last.next_id), (Some(3), None));
        let standalone = db.reader_navigation(7, "h7").unwrap().unwrap();
        assert_eq!(
            (
                standalone.series_id,
                standalone.previous_id,
                standalone.next_id
            ),
            (None, None, None)
        );
    }
}
