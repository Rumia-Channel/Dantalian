use super::book_metadata::BookMetadata;
use crate::db::NewAuthor;
use crate::open_library::{
    SEARCH_ENDPOINT, SEARCH_FIELDS, SEARCH_LIMIT, parse_search_results, search_query, user_agent,
};
use reqwest::Client;
use std::borrow::Cow;
use std::sync::LazyLock;

static USER_AGENT: LazyLock<Cow<'static, str>> = LazyLock::new(|| {
    let contact = std::env::var("OPEN_LIBRARY_CONTACT").ok();
    user_agent(contact.as_deref())
});

pub(super) async fn lookup_isbn(
    client: &Client,
    variants: &[String],
) -> Result<Option<BookMetadata>, String> {
    let mut url = reqwest::Url::parse(SEARCH_ENDPOINT)
        .map_err(|error| format!("Open Library URL failed: {error}"))?;
    url.query_pairs_mut()
        .append_pair("q", &search_query(variants))
        .append_pair("fields", SEARCH_FIELDS)
        .append_pair("limit", SEARCH_LIMIT);
    // Share the existing inter-process permit with NDL. Two seconds between
    // complete responses is also below Open Library's unidentified 1 req/s cap.
    let _guard = super::ndl::access::acquire().await?;
    let body = client
        .get(url)
        .header("User-Agent", USER_AGENT.as_ref())
        .header("Accept", "application/json")
        .timeout(super::ndl::access::REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("Open Library request failed: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("Open Library response read failed: {error}"))?;
    Ok(
        parse_search_results(&body, variants)?.map(|book| BookMetadata {
            title: book.title,
            publisher: book.publisher,
            publish_date: book.publish_date,
            authors: book
                .authors
                .into_iter()
                .map(|name| NewAuthor {
                    name,
                    ndl_id: None,
                    transcription: None,
                })
                .collect(),
            ..BookMetadata::default()
        }),
    )
}
