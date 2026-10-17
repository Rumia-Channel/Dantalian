use super::{normalize_date, parse_search_results};

const FIXTURE: &[u8] = include_bytes!("../../worker/tests/fixtures/open_library_search.json");

#[test]
fn uses_matching_edition_not_aggregate_work_metadata() {
    let variants = crate::amazon::isbn_lookup_variants("0140328726");
    let book = parse_search_results(FIXTURE, &variants).unwrap().unwrap();
    assert_eq!(book.title, "Fantastic Mr. Fox");
    assert_eq!(book.publisher.as_deref(), Some("Puffin"));
    assert_eq!(book.publish_date.as_deref(), Some("1988-10-01"));
    assert_eq!(book.authors, ["Roald Dahl"]);
}

#[test]
fn rejects_unrelated_editions_even_if_the_work_matches() {
    let mut response: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    response["docs"][0]["isbn"] = serde_json::json!(["9780140328721"]);
    response["docs"][0]["editions"]["docs"][0]["isbn"] = serde_json::json!(["9780262033848"]);
    let variants = crate::amazon::isbn_lookup_variants("9780140328721");
    assert!(
        parse_search_results(&serde_json::to_vec(&response).unwrap(), &variants)
            .unwrap()
            .is_none()
    );
    response["docs"][0]["editions"]["docs"][0]["isbn"] = serde_json::json!([]);
    assert!(
        parse_search_results(&serde_json::to_vec(&response).unwrap(), &variants)
            .unwrap()
            .is_none()
    );
}

#[test]
fn distinguishes_empty_results_from_invalid_json_and_missing_result_schema() {
    let variants = crate::amazon::isbn_lookup_variants("9780140328721");
    assert!(
        parse_search_results(br#"{"docs":[]}"#, &variants)
            .unwrap()
            .is_none()
    );
    assert!(parse_search_results(br#"{"docs":["#, &variants).is_err());
    assert!(parse_search_results(br#"{"error":"unavailable"}"#, &variants).is_err());
}

#[test]
fn preserves_date_precision_without_inventing_a_day_or_first_publication_date() {
    for (input, expected) in [
        ("October 1, 1988", Some("1988-10-01")),
        ("1 Oct 1988", Some("1988-10-01")),
        ("October 1988", Some("1988-10-NN")),
        ("1988", Some("1988-NN-NN")),
        ("1988-10-NN", Some("1988-10-NN")),
        ("February 30, 2024", None),
        ("unknown", None),
    ] {
        assert_eq!(normalize_date(input).as_deref(), expected, "{input}");
    }
}
