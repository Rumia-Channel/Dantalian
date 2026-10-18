use super::book_metadata::BookMetadata;
use crate::ndl_search::{MAXIMUM_RECORDS, RECORD_SCHEMA, SRU_ENDPOINT, isbn_query};
use reqwest::Client;
use tracing::debug;

pub(crate) mod access;
pub(crate) mod authority;
mod sru;

#[cfg(test)]
mod tests;

pub(crate) async fn lookup_ndl(
    client: &Client,
    isbn_variants: &[String],
) -> Result<Option<BookMetadata>, String> {
    let mut failure = None;
    for isbn in isbn_variants {
        match lookup_variant(client, isbn).await {
            Ok(Some(book)) => return Ok(Some(book)),
            Ok(None) => {}
            Err(error) => failure = Some(error),
        }
    }
    failure.map_or(Ok(None), Err)
}

async fn lookup_variant(client: &Client, isbn: &str) -> Result<Option<BookMetadata>, String> {
    let raw_query = isbn_query(isbn);
    let query = urlencoding::encode(&raw_query);
    let url = format!(
        "{SRU_ENDPOINT}?operation=searchRetrieve&version=1.2&recordSchema={RECORD_SCHEMA}&onlyBib=true&maximumRecords={MAXIMUM_RECORDS}&startRecord=1&recordPacking=xml&query={}",
        query
    );
    let _guard = access::acquire().await?;
    debug!(%isbn, "NDL search: {}", url);
    let body = client
        .get(&url)
        .header(
            "User-Agent",
            concat!(
                "Dantalian/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/Rumia-Channel/Dantalian)"
            ),
        )
        .timeout(access::REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("NDL request failed: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("NDL read failed: {}", e))?;
    let xml =
        std::str::from_utf8(&body).map_err(|error| format!("NDL XML decode failed: {error}"))?;
    Ok(sru::parse_candidates(xml)?
        .into_iter()
        .max_by_key(ndl_score))
}

fn ndl_score(info: &BookMetadata) -> i32 {
    let author_id_count = info.authors.iter().filter(|a| a.ndl_id.is_some()).count() as i32;
    let ndl_bib_record = info
        .ndl_url
        .as_deref()
        .is_some_and(|url| url.contains("R100000002")) as i32;
    let filled_fields = [
        info.publisher.as_ref(),
        info.publish_date.as_ref(),
        info.description.as_ref(),
        info.title_transcription.as_ref(),
        info.series_title.as_ref(),
        info.series_title_transcription.as_ref(),
        info.alternative.as_ref(),
        info.price.as_ref(),
        info.extent.as_ref(),
        info.jpno.as_ref(),
        info.ndl_url.as_ref(),
    ]
    .iter()
    .filter(|v| v.is_some())
    .count() as i32;

    author_id_count * 100 + ndl_bib_record * 50 + filled_fields
}
