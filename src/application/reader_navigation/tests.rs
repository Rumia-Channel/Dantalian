use super::*;

fn request(id: i64, hash: &str, returning: Option<&str>) -> NavigationRequest {
    NavigationQuery {
        book_key: Some(format!("book:{id}")),
        content_hash: Some(hash.to_owned()),
        return_to: returning.map(str::to_owned),
    }
    .validate()
    .unwrap()
}

fn book() -> NavigationRow {
    NavigationRow {
        id: 2,
        media_type: Some("book".to_owned()),
        epub_file_hash: Some("revision-2".to_owned()),
        epub_file_name: Some("book.epub".to_owned()),
        series_id: None,
        series_name: None,
        previous_id: None,
        previous_title: None,
        previous_hash: None,
        previous_file_name: None,
        next_id: None,
        next_title: None,
        next_hash: None,
        next_file_name: None,
    }
}

#[test]
fn malformed_or_noncanonical_identity_never_reaches_lookup() {
    for key in [
        "book:0",
        "book:-1",
        "book:+1",
        "book:01",
        "book:",
        "cd:1",
        "book:9223372036854775808",
    ] {
        assert_eq!(
            NavigationQuery {
                book_key: Some(key.to_owned()),
                content_hash: Some("revision".to_owned()),
                return_to: None
            }
            .validate()
            .err(),
            Some(NavigationError::InvalidRequest)
        );
    }
    assert_eq!(
        NavigationQuery {
            book_key: Some("book:1".to_owned()),
            content_hash: None,
            return_to: None
        }
        .validate()
        .err(),
        Some(NavigationError::InvalidRequest)
    );
}

#[test]
fn absent_changed_or_unreadable_current_book_does_not_return_context() {
    assert_eq!(
        resolve(None, request(2, "revision-2", None)).unwrap_err(),
        NavigationError::BookNotFound
    );
    assert_eq!(
        resolve(Some(book()), request(2, "old-revision", None)).unwrap_err(),
        NavigationError::ContentChanged
    );
    let mut pdf = book();
    pdf.epub_file_name = Some("book.PDF".to_owned());
    assert_eq!(
        resolve(Some(pdf), request(2, "revision-2", None)).unwrap_err(),
        NavigationError::BookUnavailable
    );
    let mut missing = book();
    missing.epub_file_hash = None;
    assert_eq!(
        resolve(Some(missing), request(2, "revision-2", None)).unwrap_err(),
        NavigationError::BookUnavailable
    );
    let mut audio = book();
    audio.media_type = Some("audiobook".to_owned());
    assert_eq!(
        resolve(Some(audio), request(2, "revision-2", None)).unwrap_err(),
        NavigationError::BookUnavailable
    );
}

#[test]
fn return_path_is_app_local_and_preserves_search_and_fragment() {
    let path = "/books/?search=作品&view=grid#selected";
    for bad in [
        "//other.example/",
        "https://other.example/books/",
        "/admin/",
        "/books/../admin/",
        "/\\other.example/",
        "/books/\n",
    ] {
        assert_eq!(safe_return_path(Some(bad)), "/books/");
    }
    let mut row = book();
    row.series_id = Some(1);
    row.series_name = Some("Series".to_owned());
    row.previous_id = Some(1);
    row.previous_title = Some("第1巻".to_owned());
    row.previous_hash = Some("revision-1".to_owned());
    row.previous_file_name = Some("one.epub".to_owned());
    row.next_id = Some(3);
    row.next_title = Some("第3巻".to_owned());
    row.next_hash = Some("revision-3".to_owned());
    row.next_file_name = Some("three.epub".to_owned());
    let context = resolve(Some(row), request(2, "revision-2", Some(path))).unwrap();
    assert_eq!(
        context.previous.unwrap().href.as_deref(),
        Some(
            "/reader/?book=1&entry=end&from=%2Fbooks%2F%3Fsearch%3D%E4%BD%9C%E5%93%81%26view%3Dgrid%23selected"
        )
    );
    assert_eq!(
        context.next.unwrap().href.as_deref(),
        Some(
            "/reader/?book=3&entry=start&from=%2Fbooks%2F%3Fsearch%3D%E4%BD%9C%E5%93%81%26view%3Dgrid%23selected"
        )
    );
}

#[test]
fn an_existing_unreadable_neighbor_is_not_a_missing_neighbor() {
    let mut row = book();
    row.series_id = Some(1);
    row.series_name = Some("Series".to_owned());
    row.previous_id = Some(1);
    row.previous_title = Some("未登録の巻".to_owned());
    row.next_id = Some(3);
    row.next_title = Some("PDF の巻".to_owned());
    row.next_hash = Some("revision-3".to_owned());
    row.next_file_name = Some("three.pdf".to_owned());
    let context = resolve(Some(row), request(2, "revision-2", None)).unwrap();
    let previous = context.previous.unwrap();
    assert_eq!(previous.status, "missing-file");
    assert!(previous.href.is_none());
    let next = context.next.unwrap();
    assert_eq!(next.status, "unsupported-format");
    assert!(next.href.is_none());
}
