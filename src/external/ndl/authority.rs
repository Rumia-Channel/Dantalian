use super::access;
use crate::db::NewAuthor;
use crate::ndl_authority::{
    AUTHORITY_BATCH_SIZE, AUTHORITY_ENDPOINT, build_authority_query, parse_authority_results,
};
use reqwest::Client;
use tracing::warn;

pub(crate) async fn enrich(client: &Client, authors: &mut [NewAuthor]) {
    let missing: Vec<usize> = authors
        .iter()
        .enumerate()
        .filter(|(_, author)| author.ndl_id.is_none() && !author.name.trim().is_empty())
        .map(|(index, _)| index)
        .collect();
    for batch in missing.chunks(AUTHORITY_BATCH_SIZE) {
        let query = {
            let names: Vec<&str> = batch
                .iter()
                .map(|&index| authors[index].name.as_str())
                .collect();
            build_authority_query(&names)
        };
        match lookup(client, &query, batch.len()).await {
            Ok(ids) => {
                for (&index, id) in batch.iter().zip(ids) {
                    authors[index].ndl_id = id;
                }
            }
            Err(error) => warn!(
                author_count = batch.len(),
                "NDL authority lookup failed: {error}"
            ),
        }
    }
}

pub(super) async fn lookup(
    client: &Client,
    query: &str,
    author_count: usize,
) -> Result<Vec<Option<String>>, String> {
    let _guard = access::acquire().await?;
    let body = client
        .post(AUTHORITY_ENDPOINT)
        .header("Accept", "application/sparql-results+json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header(
            "User-Agent",
            concat!(
                "Dantalian/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/Rumia-Channel/Dantalian)"
            ),
        )
        .timeout(access::REQUEST_TIMEOUT)
        .body(format!("query={}&format=json", urlencoding::encode(query)))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("NDL authority request failed: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("NDL authority response read failed: {error}"))?;
    let json = std::str::from_utf8(&body)
        .map_err(|error| format!("NDL authority JSON decode failed: {error}"))?;
    parse_authority_results(json, author_count)
}
