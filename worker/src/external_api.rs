use futures_util::StreamExt;
use futures_util::future::{Either, select};
use futures_util::pin_mut;
use quick_xml::{Reader, events::Event};
use std::time::Duration;
use url::Url;
use worker::{AbortController, Delay, Fetch, Response, Result};

pub(crate) mod ndl;
mod ndl_sru;
mod open_library;

const EXTERNAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// External response bodies are read with this cap. The send timeout only
/// covers the headers, so the body read itself must stay bounded as well.
pub(crate) const MAX_EXTERNAL_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct NdlAuthor {
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NdlBook {
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub alternative: Option<String>,
    pub alternative_transcription: Option<String>,
    pub volume: Option<String>,
    pub volume_transcription: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub jpno: Option<String>,
    pub ndl_url: Option<String>,
    pub cover_url: Option<String>,
    pub isdn_region: Option<String>,
    pub isdn_class: Option<String>,
    pub isdn_type: Option<String>,
    pub isdn_rating_gender: Option<String>,
    pub isdn_rating_age: Option<String>,
    pub isdn_genre_code: Option<String>,
    pub isdn_genre_name: Option<String>,
    pub isdn_genre_user: Option<String>,
    pub isdn_c_code: Option<String>,
    pub isdn_author: Option<String>,
    pub isdn_shape: Option<String>,
    pub isdn_contents: Option<String>,
    pub isdn_barcode2: Option<String>,
    pub isdn_sample_image_url: Option<String>,
    pub isdn_useroption: Option<String>,
    pub isdn_external_links: Option<String>,
    pub authors: Vec<NdlAuthor>,
}

pub(crate) async fn fetch_with_timeout(fetch: Fetch, label: &str) -> Result<worker::Response> {
    let controller = AbortController::default();
    let signal = controller.signal();
    let request = fetch.send_with_signal(&signal);
    let delay = Delay::from(EXTERNAL_REQUEST_TIMEOUT);
    pin_mut!(request, delay);

    match select(request, delay).await {
        Either::Left((result, _delay)) => result
            .map_err(|error| worker::Error::RustError(format!("{label} request failed: {error}"))),
        Either::Right((_timeout, request)) => {
            drop(request);
            controller.abort();
            Err(worker::Error::RustError(format!(
                "{label} request timed out after {} seconds",
                EXTERNAL_REQUEST_TIMEOUT.as_secs()
            )))
        }
    }
}

/// Read an external response body as text with a hard size cap.
pub(crate) async fn read_text_limited(response: Response, label: &str) -> Result<String> {
    let bytes = read_bytes_limited(response, label, MAX_EXTERNAL_RESPONSE_BYTES).await?;
    String::from_utf8(bytes).map_err(|error| {
        worker::Error::RustError(format!("{label} response was not valid UTF-8: {error}"))
    })
}

/// Read an external response body as bytes, failing when it exceeds `limit`.
pub(crate) async fn read_bytes_limited(
    response: Response,
    label: &str,
    limit: usize,
) -> Result<Vec<u8>> {
    read_bytes_capped(response, limit)
        .await?
        .ok_or_else(|| response_too_large(label, limit))
}

/// Read an external response body incrementally so a hostile or oversized
/// response cannot buffer without bound in the Worker isolate. A declared
/// content length above the cap is rejected before any body is read. Returns
/// `Ok(None)` when the body exceeds `limit`.
pub(crate) async fn read_bytes_capped(
    mut response: Response,
    limit: usize,
) -> Result<Option<Vec<u8>>> {
    let declared_length = response
        .headers()
        .get("content-length")?
        .and_then(|value| value.trim().parse::<u64>().ok());
    if declared_length.is_some_and(|length| length > limit as u64) {
        return Ok(None);
    }
    match response.stream() {
        Ok(stream) => {
            pin_mut!(stream);
            let mut body = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                if body.len() + chunk.len() > limit {
                    return Ok(None);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(Some(body))
        }
        // In-process responses (for example Durable Object stub replies) may
        // already be buffered, so the cap is enforced after the read instead.
        Err(_) => {
            let body = response.bytes().await?;
            if body.len() > limit {
                return Ok(None);
            }
            Ok(Some(body))
        }
    }
}

fn response_too_large(label: &str, limit: usize) -> worker::Error {
    worker::Error::RustError(format!(
        "{label} response body exceeded the {limit} byte limit"
    ))
}

pub(crate) async fn lookup_isbn(
    env: &worker::Env,
    isbn: &str,
) -> Result<Option<(NdlBook, &'static str)>> {
    let variants = dantalian::amazon::isbn_lookup_variants(isbn);
    match ndl::lookup_isbn(env, &variants).await? {
        Some(book) => Ok(Some((book, "ndl"))),
        None => Ok(open_library::lookup_isbn(env, &variants)
            .await?
            .map(|book| (book, "open_library"))),
    }
}

pub async fn lookup_isdn(isdn: &str) -> Result<Option<NdlBook>> {
    let url = Url::parse(&format!("https://isdn.jp/xml/{isdn}"))
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    let response = fetch_with_timeout(Fetch::Url(url), "ISDN").await?;
    if !response.status_code().to_string().starts_with('2') {
        return Ok(None);
    }
    let xml = read_text_limited(response, "ISDN").await?;
    parse_isdn(&xml).map_err(|error| worker::Error::RustError(error.to_string()))
}

fn local_name(name: quick_xml::name::QName<'_>) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).to_string()
}
fn parse_isdn(xml: &str) -> std::result::Result<Option<NdlBook>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut in_item = false;
    let mut result = NdlBook::default();
    let mut price_unit = None;
    let mut product_style = None;
    let mut product_size = None;
    let mut product_capacity = None;
    let mut product_capacity_unit = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name());
                if tag == "item" && !in_item {
                    in_item = true;
                    path.clear();
                    path.push(tag);
                } else if in_item {
                    path.push(tag);
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name());
                if in_item && tag == "item" && path.len() == 1 {
                    break;
                }
                if in_item && path.last().is_some_and(|value| value == &tag) {
                    path.pop();
                }
            }
            Ok(Event::Text(event)) if in_item => {
                let text = event
                    .decode()
                    .map_err(|error| format!("ISDN XML text decode failed: {error}"))?
                    .trim()
                    .to_string();
                if text.is_empty() || path.len() < 2 {
                    buf.clear();
                    continue;
                }
                match path[1].as_str() {
                    "product-name" if result.title.is_empty() => result.title = text,
                    "product-yomi" if result.title_transcription.is_none() => {
                        result.title_transcription = Some(text)
                    }
                    "publisher-name" if result.publisher.is_none() => result.publisher = Some(text),
                    "issue-date" if result.publish_date.is_none() => {
                        result.publish_date = Some(text)
                    }
                    "product-comment" if result.description.is_none() => {
                        result.description = Some(text)
                    }
                    "price" if result.price.is_none() => result.price = Some(text),
                    "price-unit" if price_unit.is_none() => price_unit = Some(text),
                    "product-style" if product_style.is_none() => product_style = Some(text),
                    "product-size" if product_size.is_none() => product_size = Some(text),
                    "product-capacity" if product_capacity.is_none() => {
                        product_capacity = Some(text)
                    }
                    "product-capacity-unit" if product_capacity_unit.is_none() => {
                        product_capacity_unit = Some(text)
                    }
                    "sample-image-uri" if result.cover_url.is_none() => {
                        result.cover_url = Some(text.clone());
                        result.isdn_sample_image_url = Some(text);
                    }
                    "region" if result.isdn_region.is_none() => result.isdn_region = Some(text),
                    "class" if result.isdn_class.is_none() => result.isdn_class = Some(text),
                    "type" if result.isdn_type.is_none() => result.isdn_type = Some(text),
                    "rating_gender" if result.isdn_rating_gender.is_none() => {
                        result.isdn_rating_gender = Some(text)
                    }
                    "rating_age" if result.isdn_rating_age.is_none() => {
                        result.isdn_rating_age = Some(text)
                    }
                    "genre-code" if result.isdn_genre_code.is_none() => {
                        result.isdn_genre_code = Some(text)
                    }
                    "genre-name" if result.isdn_genre_name.is_none() => {
                        result.isdn_genre_name = Some(text)
                    }
                    "genre-user" if result.isdn_genre_user.is_none() => {
                        result.isdn_genre_user = Some(text)
                    }
                    "c-code" if result.isdn_c_code.is_none() => result.isdn_c_code = Some(text),
                    "author" if result.isdn_author.is_none() => {
                        result.authors.push(NdlAuthor {
                            ndl_id: None,
                            name: text.clone(),
                            transcription: None,
                        });
                        result.isdn_author = Some(text);
                    }
                    "shape" if result.isdn_shape.is_none() => result.isdn_shape = Some(text),
                    "contents" if result.isdn_contents.is_none() => {
                        result.isdn_contents = Some(text)
                    }
                    "barcode2" if result.isdn_barcode2.is_none() => {
                        result.isdn_barcode2 = Some(text)
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("ISDN XML parse failed: {error}")),
        }
        buf.clear();
    }

    if result.title.is_empty() {
        return Ok(None);
    }
    if result.price.is_some() {
        if let (Some(price), Some(unit)) = (result.price.take(), price_unit) {
            result.price = Some(format!("{price} {unit}"));
        }
    }
    let extent = [
        product_style,
        product_size,
        product_capacity
            .zip(product_capacity_unit)
            .map(|(value, unit)| format!("{value}{unit}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !extent.is_empty() {
        result.extent = Some(extent.join(", "));
        if result.description.is_none() {
            result.description = result.extent.clone();
        }
    }
    result.publish_date = result.publish_date.take().and_then(|value| {
        dantalian::application::publish_date::normalize_publish_date(Some(&value))
    });

    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::parse_isdn;

    #[test]
    fn parses_isdn_fields() {
        let xml = r#"
            <response><item>
              <product-name>Media title</product-name>
              <publisher-name>Publisher</publisher-name>
              <issue-date>2025.11</issue-date>
              <price>1200</price><price-unit>円</price-unit>
              <product-style>Paperback</product-style>
              <product-size>A5</product-size>
              <author>Author</author>
              <sample-image-uri>https://example.test/cover.jpg</sample-image-uri>
            </item></response>
        "#;
        let parsed = parse_isdn(xml).unwrap().unwrap();
        assert_eq!(parsed.publish_date.as_deref(), Some("2025-11-NN"));
        assert_eq!(parsed.title, "Media title");
        assert_eq!(parsed.price.as_deref(), Some("1200 円"));
        assert_eq!(parsed.extent.as_deref(), Some("Paperback, A5"));
        assert_eq!(parsed.isdn_author.as_deref(), Some("Author"));
        assert_eq!(
            parsed.cover_url.as_deref(),
            Some("https://example.test/cover.jpg")
        );
    }
}
