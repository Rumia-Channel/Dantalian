use dantalian::amazon::{
    AmazonInfo, amazon_detail_url, amazon_info_is_acceptable, amazon_info_matches_isbn,
    amazon_isbn_detail_url, amazon_metadata_is_verified, amazon_search_urls,
    black_curtain_eligibility_url, image_content_type as shared_image_content_type,
    is_allowed_amazon_url, isbn_lookup_variants, needs_black_curtain_eligibility,
    parse_amazon_detail, parse_amazon_search_results as shared_parse_amazon_search_results,
};
#[cfg(test)]
use dantalian::amazon::{
    amazon_search_url as shared_amazon_search_url, parse_amazon_image_url,
    parse_amazon_search_result as shared_parse_amazon_search_result,
};
use sha2::{Digest, Sha256};
#[cfg(test)]
use url::Url;
use worker::{Fetch, Headers, Method, Request, RequestInit, Result, RouteContext};

use crate::{
    external_api::fetch_with_timeout,
    wasabi::{WasabiConfig, WasabiStorage},
};
use dantalian::ports::object_storage::{ObjectKind, object_key};

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

pub(crate) struct AmazonMetadata {
    pub(crate) info: AmazonInfo,
    pub(crate) metadata_verified: bool,
    pub(crate) cover: Option<AmazonCover>,
}

#[cfg(test)]
fn parse_amazon_search_result(html: &str) -> Option<String> {
    shared_parse_amazon_search_result(html)
}

fn parse_amazon_search_results(html: &str) -> Vec<String> {
    shared_parse_amazon_search_results(html)
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
async fn lookup_amazon_info(
    search_keys: &[&str],
    fallback_isbn: Option<&str>,
) -> Result<Option<AmazonInfo>> {
    const MAX_AMAZON_DETAIL_ATTEMPTS: usize = 12;
    let expected_isbn13 = fallback_isbn.and_then(|isbn| {
        isbn_lookup_variants(isbn)
            .into_iter()
            .find(|variant| variant.len() == 13)
    });
    let mut attempted_details = Vec::new();
    'search: for search_url in amazon_search_urls(search_keys) {
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
        for href in parse_amazon_search_results(&search_html) {
            let Some(detail_url) = amazon_detail_url_for_request(&href) else {
                continue;
            };
            if attempted_details.iter().any(|url| url == &detail_url) {
                continue;
            }
            if attempted_details.len() >= MAX_AMAZON_DETAIL_ATTEMPTS {
                break 'search;
            }
            attempted_details.push(detail_url.clone());
            let detail_html = match fetch_detail_html(&detail_url).await {
                Ok(Some(detail_html)) => detail_html,
                Ok(None) => continue,
                Err(error) => {
                    worker::console_error!("Amazon detail lookup failed: {error}");
                    continue;
                }
            };
            let info = parse_amazon_detail(&detail_html);
            if amazon_info_is_acceptable(&info, expected_isbn13.as_deref()) {
                return Ok(Some(info));
            }
        }
    }

    let Some(fallback_isbn) = fallback_isbn else {
        return Ok(None);
    };
    let Some(detail_url) = amazon_isbn_detail_url(fallback_isbn) else {
        return Ok(None);
    };
    if attempted_details.iter().any(|url| url == &detail_url) {
        return Ok(None);
    }
    let Some(detail_html) = fetch_detail_html(&detail_url).await? else {
        return Ok(None);
    };
    let info = parse_amazon_detail(&detail_html);
    if amazon_info_is_acceptable(&info, expected_isbn13.as_deref()) {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

async fn fetch_cover(url: &str) -> Result<Option<AmazonCover>> {
    if !is_allowed_amazon_url(url) {
        return Ok(None);
    }
    let mut image_response =
        fetch_with_timeout(Fetch::Request(amazon_request(url)?), "Amazon cover").await?;
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

pub(crate) async fn lookup_amazon_metadata(
    isbn: &str,
    search_terms: &[&str],
) -> Result<Option<AmazonMetadata>> {
    let mut keys = Vec::with_capacity(search_terms.len() + 1);
    keys.push(isbn);
    keys.extend(search_terms.iter().copied());
    let Some(info) = lookup_amazon_info(&keys, Some(isbn)).await? else {
        return Ok(None);
    };
    let variants = isbn_lookup_variants(isbn);
    let expected_isbn13 = variants
        .iter()
        .find(|variant| variant.len() == 13)
        .map(String::as_str);
    if !amazon_info_matches_isbn(&info, expected_isbn13) {
        return Ok(None);
    }
    let metadata_verified = amazon_metadata_is_verified(&info, expected_isbn13);
    let cover = match info.cover_url.as_deref() {
        Some(url) => fetch_cover(url).await?,
        None => None,
    };
    Ok(Some(AmazonMetadata {
        info,
        metadata_verified,
        cover,
    }))
}

pub(crate) async fn lookup_amazon_title_for_jan(jan: &str) -> Result<Option<String>> {
    Ok(lookup_amazon_info(&[jan], None)
        .await?
        .and_then(|info| info.title)
        .filter(|title| !title.trim().is_empty()))
}

pub(crate) async fn lookup_amazon_cover_for_jan(jan: &str) -> Result<Option<AmazonCover>> {
    let Some(info) = lookup_amazon_info(&[jan], None).await? else {
        return Ok(None);
    };
    match info.cover_url.as_deref() {
        Some(url) => fetch_cover(url).await,
        None => Ok(None),
    }
}

pub(crate) async fn persist_cover(ctx: &RouteContext<()>, cover: &AmazonCover) -> Result<String> {
    let config = WasabiConfig::from_env(&ctx.env).await?;
    let file_name = format!("{}.{}", cover.object_id, cover.extension);
    let key = object_key(
        config.prefix.as_deref(),
        ObjectKind::CoverImage,
        &cover.object_id,
        &cover.extension,
    )
    .map_err(|error| worker::Error::from(error.to_string()))?;
    WasabiStorage::new(config)
        .put_object(&key, &cover.content_type, &cover.bytes)
        .await
        .map_err(|error| worker::Error::from(error.to_string()))?;
    Ok(file_name)
}

#[cfg(test)]
mod tests {
    use super::{
        amazon_detail_url_for_request, amazon_search_url, image_content_type,
        parse_amazon_image_url, parse_amazon_search_result, parse_amazon_search_results,
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
    fn skips_untyped_kindle_link_for_physical_book() {
        let html = r#"
            <div data-component-type="s-search-result">
                <a href="/dp/B0FX7VSWLJ">Kindle (Digital)</a>
                <a href="/dp/4041164699">Hardcover</a>
            </div>
        "#;
        assert_eq!(parse_amazon_search_results(html), vec!["/dp/4041164699"]);
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
