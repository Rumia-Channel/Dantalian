#[cfg(test)]
use dantalian::amazon::amazon_search_url as shared_amazon_search_url;
use dantalian::amazon::{
    amazon_detail_url, amazon_isbn_detail_url, amazon_search_urls, black_curtain_eligibility_url,
    image_content_type as shared_image_content_type, is_allowed_amazon_url,
    needs_black_curtain_eligibility, parse_amazon_image_url,
    parse_amazon_search_result as shared_parse_amazon_search_result,
};
use sha2::{Digest, Sha256};
#[cfg(test)]
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

fn amazon_request(url: &str) -> Result<Request> {
    let headers = Headers::new();
    headers.set("User-Agent", AMAZON_USER_AGENT)?;
    headers.set("Accept-Language", "ja-JP,ja;q=0.9,en-US;q=0.8,en;q=0.7")?;
    headers.set(
        "Accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
    )?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    Request::new_with_init(url, &init)
}

fn parse_amazon_search_result(html: &str) -> Option<String> {
    shared_parse_amazon_search_result(html)
}

#[cfg(test)]
fn amazon_search_url(key: &str) -> Option<Url> {
    let value = shared_amazon_search_url(key)?;
    Url::parse(&value).ok()
}

fn amazon_detail_url_for_request(href: &str) -> Option<String> {
    amazon_detail_url(href)
}

fn image_content_type(value: &str) -> Option<(&'static str, &'static str)> {
    shared_image_content_type(value)
}

fn image_url_from_detail(html: &str) -> Option<String> {
    parse_amazon_image_url(html).filter(|url| is_allowed_amazon_url(url))
}

async fn fetch_detail_html(detail_url: &str) -> Result<Option<String>> {
    let mut detail_response =
        fetch_with_timeout(Fetch::Request(amazon_request(detail_url)?), "Amazon detail").await?;
    if !(200..300).contains(&detail_response.status_code()) {
        return Ok(None);
    }
    let detail_html = detail_response.text().await?;
    if !needs_black_curtain_eligibility(&detail_html) {
        return Ok(Some(detail_html));
    }

    let eligibility_url = black_curtain_eligibility_url(detail_url);
    let mut eligibility_response = fetch_with_timeout(
        Fetch::Request(amazon_request(&eligibility_url)?),
        "Amazon age eligibility",
    )
    .await?;
    let _ = eligibility_response.text().await?;

    let mut retry_response = fetch_with_timeout(
        Fetch::Request(amazon_request(detail_url)?),
        "Amazon detail retry",
    )
    .await?;
    if !(200..300).contains(&retry_response.status_code()) {
        return Ok(None);
    }
    Ok(Some(retry_response.text().await?))
}

pub(crate) async fn lookup_amazon_cover(
    isbn: &str,
    search_terms: &[&str],
) -> Result<Option<AmazonCover>> {
    let mut keys = Vec::with_capacity(search_terms.len() + 1);
    keys.push(isbn);
    keys.extend(search_terms.iter().copied());

    let mut href = None;
    for search_url in amazon_search_urls(&keys) {
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
        let Ok(search_html) = search_response.text().await else {
            continue;
        };
        if let Some(found) = parse_amazon_search_result(&search_html) {
            href = Some(found);
            break;
        }
    }
    let detail_url = href
        .as_deref()
        .and_then(amazon_detail_url_for_request)
        .or_else(|| amazon_isbn_detail_url(isbn));
    let Some(detail_url) = detail_url else {
        return Ok(None);
    };
    let Some(detail_html) = fetch_detail_html(&detail_url).await? else {
        worker::console_error!("Amazon detail request returned a non-success status");
        return Ok(None);
    };
    let Some(image_url) = image_url_from_detail(&detail_html) else {
        worker::console_error!(
            "Amazon detail returned no supported cover image (response bytes: {})",
            detail_html.len()
        );
        return Ok(None);
    };
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
        amazon_detail_url_for_request, amazon_search_url, image_content_type,
        parse_amazon_image_url, parse_amazon_search_result,
    };

    #[test]
    fn builds_native_stripbooks_search_url() {
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
            amazon_detail_url_for_request("/gp/aw/d/B0FX7VSWLJ").as_deref(),
            Some("https://www.amazon.co.jp/gp/aw/d/B0FX7VSWLJ")
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
