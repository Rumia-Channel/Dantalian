use sha2::{Digest, Sha256};
use url::Url;
use worker::{Fetch, Headers, Method, Request, RequestInit, Result};

use crate::external_api::fetch_with_timeout;

const AMAZON_MAX_COVER_BYTES: usize = 10 * 1024 * 1024;
const AMAZON_USER_AGENT: &str =
    "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 Chrome/131.0 Mobile Safari/537.36";

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

fn amazon_mobile_search_url(value: &str, path: &str, query: &str) -> Result<Url> {
    let mut url = Url::parse(&format!("https://www.amazon.co.jp{path}"))
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair(query, value)
        .append_pair("i", "stripbooks");
    Ok(url)
}

fn isbn10_for_amazon(isbn: &str) -> Option<String> {
    let digits: String = isbn.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 13 || !digits.starts_with("978") {
        return None;
    }
    let body = &digits[3..12];
    let sum: u32 = body
        .chars()
        .enumerate()
        .map(|(index, digit)| (10 - index as u32) * digit.to_digit(10).unwrap_or(0))
        .sum();
    let check = match 11 - (sum % 11) {
        10 => 'X',
        11 => '0',
        value => char::from_digit(value, 10)?,
    };
    Some(format!("{body}{check}"))
}

fn amazon_search_urls(isbn: &str, search_terms: &[&str]) -> Result<Vec<Url>> {
    let digits: String = isbn.chars().filter(char::is_ascii_digit).collect();
    let mut keys = search_terms
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if digits.len() == 13 && digits.starts_with("978") {
        let legacy = format!("{}{}", &digits[3..12], &digits[12..]);
        if !keys.contains(&legacy) {
            keys.push(legacy);
        }
        if let Some(isbn10) = isbn10_for_amazon(isbn) {
            if !keys.contains(&isbn10) {
                keys.push(isbn10);
            }
        }
    }
    if !keys.contains(&isbn.to_string()) {
        keys.push(isbn.to_string());
    }
    let mut urls = Vec::new();
    for key in keys {
        urls.push(amazon_mobile_search_url(&key, "/gp/search/", "keywords")?);
        urls.push(amazon_mobile_search_url(&key, "/gp/aw/s", "k")?);
    }
    Ok(urls)
}

fn is_physical_product_href(href: &str) -> bool {
    let path = href
        .split(['?', '#'])
        .next()
        .unwrap_or(href)
        .to_ascii_lowercase();
    if path.starts_with("http") && !path.starts_with("https://www.amazon.co.jp/") {
        return false;
    }
    let Some((product_path, _)) = path
        .split_once("/dp/")
        .or_else(|| path.split_once("/gp/aw/d/"))
    else {
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
    first_physical_product_href(html)
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

fn extract_json_string(html: &str, marker: &str) -> Option<String> {
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find('"').map(|offset| start + offset)?;
    let value = html[start..end].replace("\\/", "/").replace("\\u0026", "&");
    (!value.is_empty()).then_some(value)
}

fn parse_amazon_image_url(html: &str) -> Option<String> {
    for marker in [r#""landingImageUrl":""#, "landingImageUrl\":\""] {
        if let Some(url) = valid_amazon_image_url(extract_json_string(html, marker)) {
            return Some(url);
        }
    }

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

fn amazon_detail_url(href: &str) -> Result<Url> {
    let absolute = if href.starts_with('/') {
        format!("https://www.amazon.co.jp{href}")
    } else {
        href.to_string()
    };
    let parsed =
        Url::parse(&absolute).map_err(|error| worker::Error::RustError(error.to_string()))?;
    let path = parsed.path();
    let asin = path
        .split("/dp/")
        .nth(1)
        .or_else(|| path.split("/gp/aw/d/").nth(1))
        .and_then(|value| value.split(['/', '?', '#']).next())
        .filter(|value| !value.is_empty());
    match asin {
        Some(asin) => Url::parse(&format!("https://www.amazon.co.jp/gp/aw/d/{asin}"))
            .map_err(|error| worker::Error::RustError(error.to_string())),
        None => Ok(parsed),
    }
}

pub(crate) async fn lookup_amazon_cover(
    isbn: &str,
    search_terms: &[&str],
) -> Result<Option<AmazonCover>> {
    let mut href = None;
    for search_url in amazon_search_urls(isbn, search_terms)? {
        let Ok(mut search_response) = fetch_with_timeout(
            Fetch::Request(amazon_request(&search_url)?),
            "Amazon search",
        )
        .await
        else {
            continue;
        };
        if !(200..300).contains(&search_response.status_code()) {
            continue;
        }
        let search_html = search_response.text().await?;
        if let Some(found) = parse_amazon_search_result(&search_html) {
            href = Some(found);
            break;
        }
    }
    let Some(href) = href else {
        return Ok(None);
    };
    let detail_url = amazon_detail_url(&href)?;
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
        amazon_detail_url, image_content_type, isbn10_for_amazon, parse_amazon_image_url,
        parse_amazon_search_result,
    };

    #[test]
    fn parses_isbn10_for_amazon_search() {
        assert_eq!(
            isbn10_for_amazon("9784041164693").as_deref(),
            Some("4041164699")
        );
        assert_eq!(
            isbn10_for_amazon("9780451524935").as_deref(),
            Some("0451524934")
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
    fn selects_mobile_physical_product_link() {
        let html = r#"
            <div class="s-result-item">
                <a href="/gp/aw/d/B0FX7VSWLJ">Hardcover</a>
            </div>
        "#;

        assert_eq!(
            parse_amazon_search_result(html).as_deref(),
            Some("/gp/aw/d/B0FX7VSWLJ")
        );
        assert_eq!(
            amazon_detail_url("/gp/aw/d/B0FX7VSWLJ").unwrap().as_str(),
            "https://www.amazon.co.jp/gp/aw/d/B0FX7VSWLJ"
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

    #[test]
    fn extracts_mobile_json_cover_url() {
        let html = r#"<script>"landingImageUrl":"https:\/\/m.media-amazon.com\/images\/I\/cover.jpg"</script>"#;

        assert_eq!(
            parse_amazon_image_url(html).as_deref(),
            Some("https://m.media-amazon.com/images/I/cover.jpg")
        );
    }
}
