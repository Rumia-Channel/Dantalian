use sha2::{Digest, Sha256};
use url::Url;
use worker::{Fetch, Headers, Method, Request, RequestInit, Result};

use crate::external_api::fetch_with_timeout;

const AMAZON_MAX_COVER_BYTES: usize = 10 * 1024 * 1024;
const AMAZON_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0 Safari/537.36";

pub(crate) struct AmazonCover {
    pub(crate) object_id: String,
    pub(crate) extension: String,
    pub(crate) content_type: String,
    pub(crate) bytes: Vec<u8>,
}
fn amazon_request(url: &Url) -> Result<Request> {
    let headers = Headers::new();
    headers.set("User-Agent", AMAZON_USER_AGENT)?;
    headers.set("Accept-Language", "ja-JP,ja;q=0.9,en-US;q=0.8,en;q=0.7")?;
    headers.set(
        "Accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
    )?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    Request::new_with_init(url.as_str(), &init)
}

fn amazon_search_url(isbn: &str) -> Result<Url> {
    let mut url = Url::parse("https://www.amazon.co.jp/s")
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("k", isbn)
        .append_pair("i", "stripbooks");
    Ok(url)
}

fn is_physical_product_href(href: &str) -> bool {
    let path = href
        .split(['?', '#'])
        .next()
        .unwrap_or(href)
        .to_ascii_lowercase();
    let Some((product_path, _)) = path.split_once("/dp/") else {
        return false;
    };
    !product_path
        .split(|ch: char| matches!(ch, '/' | '-' | '_'))
        .any(|segment| matches!(segment, "ebook" | "kindle" | "digital" | "audible"))
}

fn href_marker(html: &str, cursor: usize) -> Option<(usize, char)> {
    let double = html[cursor..]
        .find("href=\"")
        .map(|offset| (cursor + offset, '"'));
    let single = html[cursor..]
        .find("href='")
        .map(|offset| (cursor + offset, '\''));
    match (double, single) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn first_physical_product_href(html: &str) -> Option<String> {
    let mut cursor = 0;
    while let Some((start, quote)) = href_marker(html, cursor) {
        let value_start = start + 6;
        let end = html[value_start..]
            .find(quote)
            .map(|offset| value_start + offset)?;
        let href = &html[value_start..end];
        if is_physical_product_href(href) {
            return Some(href.to_string());
        }
        cursor = end + 1;
    }
    None
}

fn search_card_marker(html: &str, cursor: usize) -> Option<(usize, usize)> {
    let double = html[cursor..]
        .find(r#"data-component-type="s-search-result""#)
        .map(|offset| {
            (
                cursor + offset,
                r#"data-component-type="s-search-result""#.len(),
            )
        });
    let single = html[cursor..]
        .find("data-component-type='s-search-result'")
        .map(|offset| {
            (
                cursor + offset,
                "data-component-type='s-search-result'".len(),
            )
        });
    match (double, single) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn parse_amazon_search_result(html: &str) -> Option<String> {
    let mut cursor = 0;
    while let Some((start, marker_len)) = search_card_marker(html, cursor) {
        let next_start = search_card_marker(html, start + marker_len)
            .map(|(next, _)| next)
            .unwrap_or(html.len());
        let card = &html[start..next_start];
        if card.contains("data-asin") {
            if let Some(href) = first_physical_product_href(card) {
                return Some(href);
            }
        }
        cursor = start + marker_len;
    }
    None
}

fn tag_with_id<'a>(html: &'a str, id: &str) -> Option<&'a str> {
    let double_marker = format!(r#"id="{id}""#);
    let single_marker = format!("id='{id}'");
    let position = [html.find(&double_marker), html.find(&single_marker)]
        .into_iter()
        .flatten()
        .min()?;
    let start = html[..position].rfind('<')?;
    let end = position + html[position..].find('>')? + 1;
    Some(&html[start..end])
}

fn extract_attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let marker = format!("{name}={quote}");
        let Some(marker_start) = tag.find(&marker) else {
            continue;
        };
        let start = marker_start + marker.len();
        let end = tag[start..].find(quote).map(|offset| start + offset)?;
        let value = tag[start..end].trim();
        if !value.is_empty() {
            return Some(value.replace("&amp;", "&"));
        }
    }
    None
}
fn valid_amazon_image_url(value: Option<String>) -> Option<String> {
    value.filter(|url| {
        (url.starts_with("https://") || url.starts_with("http://")) && !url.starts_with("data:")
    })
}

fn is_allowed_amazon_host(url: &Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    host == "amazon.co.jp"
        || host.ends_with(".amazon.co.jp")
        || host == "amazon.com"
        || host.ends_with(".amazon.com")
        || host.ends_with(".media-amazon.com")
        || host.ends_with(".ssl-images-amazon.com")
}

fn parse_amazon_image_url(html: &str) -> Option<String> {
    for id in ["landingImage", "imgBlkFront"] {
        let Some(tag) = tag_with_id(html, id) else {
            continue;
        };
        for attribute in ["data-old-hires", "src"] {
            if let Some(url) = valid_amazon_image_url(extract_attribute(tag, attribute)) {
                return Some(url);
            }
        }
        if let Some(dynamic) = extract_attribute(tag, "data-a-dynamic-image") {
            let decoded = dynamic.replace("&quot;", "\"");
            if let Ok(images) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&decoded)
            {
                if let Some(url) = images
                    .keys()
                    .filter(|url| !url.is_empty())
                    .max_by_key(|url| url.len())
                    .cloned()
                {
                    if let Some(url) = valid_amazon_image_url(Some(url)) {
                        return Some(url);
                    }
                }
            }
        }
    }

    let meta = [
        "property=\"og:image\"",
        "property='og:image'",
        "name=\"og:image\"",
        "name='og:image'",
    ];
    for marker in meta {
        if let Some(position) = html.find(marker) {
            let start = html[..position].rfind('<')?;
            let end = position + html[position..].find('>')? + 1;
            let tag = &html[start..end];
            if let Some(url) = valid_amazon_image_url(extract_attribute(tag, "content")) {
                return Some(url);
            }
        }
    }
    None
}

fn image_content_type(value: &str) -> Option<(&str, &str)> {
    let mime = value.split(';').next()?.trim().to_ascii_lowercase();
    match mime.as_str() {
        "image/jpeg" => Some(("image/jpeg", "jpg")),
        "image/jpg" => Some(("image/jpeg", "jpg")),
        "image/png" => Some(("image/png", "png")),
        "image/webp" => Some(("image/webp", "webp")),
        "image/gif" => Some(("image/gif", "gif")),
        _ => None,
    }
}

pub(crate) async fn lookup_amazon_cover(isbn: &str) -> Result<Option<AmazonCover>> {
    let search_url = amazon_search_url(isbn)?;
    let mut search_response = fetch_with_timeout(
        Fetch::Request(amazon_request(&search_url)?),
        "Amazon search",
    )
    .await?;
    if !(200..300).contains(&search_response.status_code()) {
        return Ok(None);
    }
    let search_html = search_response.text().await?;
    let Some(href) = parse_amazon_search_result(&search_html) else {
        return Ok(None);
    };
    let detail_url = if href.starts_with('/') {
        Url::parse(&format!("https://www.amazon.co.jp{href}"))
    } else {
        Url::parse(&href)
    }
    .map_err(|error| worker::Error::RustError(error.to_string()))?;
    if !is_allowed_amazon_host(&detail_url) {
        return Ok(None);
    }
    let mut detail_response = fetch_with_timeout(
        Fetch::Request(amazon_request(&detail_url)?),
        "Amazon detail",
    )
    .await?;
    if !(200..300).contains(&detail_response.status_code()) {
        return Ok(None);
    }
    let detail_html = detail_response.text().await?;
    let Some(image_url) = parse_amazon_image_url(&detail_html) else {
        return Ok(None);
    };
    let image_url =
        Url::parse(&image_url).map_err(|error| worker::Error::RustError(error.to_string()))?;
    if !is_allowed_amazon_host(&image_url) {
        return Ok(None);
    }
    let mut image_response =
        fetch_with_timeout(Fetch::Request(amazon_request(&image_url)?), "Amazon cover").await?;
    if !(200..300).contains(&image_response.status_code()) {
        return Ok(None);
    }
    let content_type = image_response
        .headers()
        .get("content-type")?
        .unwrap_or_default();
    let Some((content_type, extension)) = image_content_type(&content_type) else {
        return Ok(None);
    };
    if image_response
        .headers()
        .get("content-length")?
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|size| size > AMAZON_MAX_COVER_BYTES)
    {
        return Ok(None);
    }
    let bytes = image_response.bytes().await?;
    if bytes.is_empty() || bytes.len() > AMAZON_MAX_COVER_BYTES {
        return Ok(None);
    }
    let digest = Sha256::digest(&bytes);
    let object_id = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(Some(AmazonCover {
        object_id,
        extension: extension.to_string(),
        content_type: content_type.to_string(),
        bytes,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        amazon_search_url, image_content_type, parse_amazon_image_url, parse_amazon_search_result,
    };

    #[test]
    fn builds_stripbooks_search_url() {
        assert_eq!(
            amazon_search_url("9784041164693").unwrap().as_str(),
            "https://www.amazon.co.jp/s?k=9784041164693&i=stripbooks"
        );
    }

    #[test]
    fn selects_physical_product_over_ebook_variant() {
        let html = r#"
            <div data-component-type="s-search-result" data-asin="B0FX7VSWLJ">
                <a href="/secret-ebook/dp/B0FX7VSWLJ">Kindle</a>
                <a href="/secret-hardcover/dp/4041164693">Hardcover</a>
            </div>
        "#;

        assert_eq!(
            parse_amazon_search_result(html).as_deref(),
            Some("/secret-hardcover/dp/4041164693")
        );
    }

    #[test]
    fn does_not_select_ebook_only_search_result() {
        let html = r#"
            <div data-component-type="s-search-result" data-asin="B0FX7VSWLJ">
                <a href="/secret-ebook/dp/B0FX7VSWLJ">Kindle</a>
            </div>
        "#;

        assert_eq!(parse_amazon_search_result(html), None);
    }

    #[test]
    fn extracts_amazon_cover_and_content_type() {
        let html = r#"
            <img id="landingImage"
                 data-old-hires="https://images.example.test/cover.jpg"
                 src="https://images.example.test/thumbnail.jpg">
        "#;

        assert_eq!(
            parse_amazon_image_url(html).as_deref(),
            Some("https://images.example.test/cover.jpg")
        );
        assert_eq!(
            image_content_type("image/jpeg; charset=binary"),
            Some(("image/jpeg", "jpg"))
        );
    }
}
