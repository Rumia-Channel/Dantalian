use crate::{
    amazon::{self, AmazonInfo},
    db::NewBook,
};
use base64::Engine;
use reqwest::Client;
use sha3::{Digest, Sha3_256};
use tracing::{debug, warn};

fn amazon_request(client: &Client, url: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("User-Agent", ua_generator::ua::spoof_ua())
        .header("Accept-Language", "ja-JP,ja;q=0.9,en-US;q=0.8,en;q=0.7")
}

pub async fn lookup_isbn(
    client: &Client,
    isbn: &str,
    images_dir: &str,
) -> Result<Option<NewBook>, String> {
    let isbn_variants = amazon::isbn_lookup_variants(isbn);
    let Some(mut ndl) = super::ndl::lookup_ndl(client, &isbn_variants).await? else {
        return Ok(None);
    };
    super::ndl::enrich_author_ndl_ids(client, &mut ndl.authors).await;
    let expected_isbn13 = isbn_variants
        .iter()
        .find(|variant| variant.len() == 13)
        .map(String::as_str);

    let (cover_url, amazon_description, amazon_publish_date) =
        fetch_amazon_cover_with_retry(client, &isbn_variants, images_dir, isbn, expected_isbn13)
            .await;

    let description = amazon_description.or(ndl.description);

    Ok(Some(NewBook {
        isbn: Some(isbn.to_string()),
        isdn: None,
        jan: None,
        title: ndl.title,
        publisher: ndl.publisher,
        publish_date: amazon_publish_date
            .or_else(|| crate::external::normalize_publish_date(ndl.publish_date.as_deref())),
        cover_url,
        description,
        title_transcription: ndl.title_transcription,
        series_title: ndl.series_title,
        series_title_transcription: ndl.series_title_transcription,
        alternative: ndl.alternative,
        alternative_transcription: ndl.alternative_transcription,
        volume: ndl.volume,
        volume_transcription: ndl.volume_transcription,
        price: ndl.price,
        extent: ndl.extent,
        jpno: ndl.jpno,
        ndl_url: ndl.ndl_url,
        authors: ndl.authors,
        media_type: None,
        catalog_number: None,
        artist: None,
        label: None,
        disc_count: None,
        tracks: None,
        isdn_region: None,
        isdn_class: None,
        isdn_type: None,
        isdn_rating_gender: None,
        isdn_rating_age: None,
        isdn_genre_code: None,
        isdn_genre_name: None,
        isdn_genre_user: None,
        isdn_c_code: None,
        isdn_author: None,
        isdn_shape: None,
        isdn_contents: None,
        isdn_barcode2: None,
        isdn_sample_image_url: None,
        isdn_useroption: None,
        isdn_external_links: None,
    }))
}

async fn download_cover(client: &Client, url: &str, images_dir: &str) -> Result<String, String> {
    if url.is_empty() {
        return Err("Empty cover URL".to_string());
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("Invalid cover URL scheme: {}", url));
    }
    let response = amazon_request(client, url)
        .send()
        .await
        .map_err(|e| format!("Cover download failed: {}", e))?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let ext = match content_type.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "jpg",
    };

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Cover read failed: {}", e))?;

    let hash = Sha3_256::digest(&bytes);
    let filename = format!(
        "{}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash),
        ext
    );
    let filepath = std::path::Path::new(images_dir).join(&filename);

    if !filepath.exists() {
        std::fs::write(&filepath, &bytes).map_err(|e| format!("Failed to save cover: {}", e))?;
        debug!(%url, %filename, "Cover saved");
    } else {
        debug!(%url, %filename, "Cover already exists");
    }

    Ok(filename)
}

fn empty_amazon_info() -> AmazonInfo {
    AmazonInfo {
        title: None,
        cover_url: None,
        description: None,
        publish_date: None,
        isbn13: None,
    }
}

async fn fetch_amazon_detail_info(
    client: &Client,
    lookup_key: &str,
    detail_url: &str,
) -> Result<AmazonInfo, String> {
    debug!(key = %lookup_key, "Amazon detail: {}", detail_url);
    let detail_body = amazon_request(client, detail_url)
        .send()
        .await
        .map_err(|e| format!("Amazon detail request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Amazon detail read failed: {}", e))?;

    let detail_body = if amazon::needs_black_curtain_eligibility(&detail_body) {
        debug!(key = %lookup_key, "Amazon black-curtain detected, confirming age eligibility");
        let eligibility_url = amazon::black_curtain_eligibility_url(detail_url);
        amazon_request(client, &eligibility_url)
            .send()
            .await
            .map_err(|e| format!("Amazon age eligibility request failed: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Amazon age eligibility read failed: {}", e))?;

        amazon_request(client, detail_url)
            .send()
            .await
            .map_err(|e| format!("Amazon detail retry failed: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Amazon detail retry read failed: {}", e))?
    } else {
        detail_body
    };

    tokio::task::spawn_blocking(move || parse_amazon_detail(&detail_body))
        .await
        .map_err(|e| format!("Amazon detail parse panicked: {}", e))
}

async fn lookup_amazon_info(
    client: &Client,
    lookup_key: &str,
    expected_isbn13: Option<&str>,
) -> Result<AmazonInfo, String> {
    let mut saw_product = false;
    for search_url in amazon::amazon_search_urls(&[lookup_key]) {
        debug!(key = %lookup_key, "Amazon search: {}", search_url);
        let search_body = amazon_request(client, &search_url)
            .send()
            .await
            .map_err(|e| format!("Amazon request failed: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Amazon read failed: {}", e))?;

        let product_urls = tokio::task::spawn_blocking({
            let body = search_body;
            move || amazon::parse_amazon_search_results(&body)
        })
        .await
        .map_err(|e| format!("Amazon search parse panicked: {}", e))?;

        for product_url in product_urls {
            let Some(detail_url) = amazon::amazon_detail_url(&product_url) else {
                continue;
            };
            saw_product = true;
            let info = match fetch_amazon_detail_info(client, lookup_key, &detail_url).await {
                Ok(info) => info,
                Err(error) => {
                    warn!(key = %lookup_key, "Amazon detail lookup failed: {}", error);
                    continue;
                }
            };
            if amazon::amazon_info_has_expected_isbn(&info, expected_isbn13) {
                return Ok(info);
            }
            debug!(
                key = %lookup_key,
                expected_isbn13 = ?expected_isbn13,
                actual_isbn13 = ?info.isbn13,
                "Skipping Amazon product without the requested ISBN-13"
            );
        }
    }

    if let Some(detail_url) = amazon::amazon_isbn_detail_url(lookup_key) {
        if let Ok(info) = fetch_amazon_detail_info(client, lookup_key, &detail_url).await {
            if amazon::amazon_info_has_expected_isbn(&info, expected_isbn13) {
                return Ok(info);
            }
        }
    }

    if !saw_product {
        warn!(key = %lookup_key, "No product link found in Amazon search results");
    }
    Ok(empty_amazon_info())
}

async fn lookup_amazon_cover(
    client: &Client,
    lookup_key: &str,
    images_dir: &str,
    expected_isbn13: Option<&str>,
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    let amazon_info = lookup_amazon_info(client, lookup_key, expected_isbn13).await?;
    let metadata_verified = amazon::amazon_metadata_is_verified(&amazon_info, expected_isbn13);
    if !amazon::amazon_info_matches_isbn(&amazon_info, expected_isbn13) {
        warn!(
            key = %lookup_key,
            expected_isbn13 = ?expected_isbn13,
            actual_isbn13 = ?amazon_info.isbn13,
            "Amazon ISBN-13 did not match; ignoring product metadata and cover"
        );
        return Ok((None, None, None));
    }
    if expected_isbn13.is_some() && !metadata_verified {
        warn!(
            key = %lookup_key,
            expected_isbn13 = ?expected_isbn13,
            "Amazon detail has no ISBN-13; keeping cover but ignoring product metadata"
        );
    }
    let description = metadata_verified
        .then(|| amazon_info.description.clone())
        .flatten();
    let publish_date = metadata_verified
        .then(|| amazon_info.publish_date.clone())
        .flatten();
    match &amazon_info.cover_url {
        Some(url) => debug!(key = %lookup_key, "Amazon cover found: {}", url),
        None => warn!(key = %lookup_key, "No cover image found on Amazon detail page"),
    }
    if description.is_some() {
        debug!(key = %lookup_key, "Amazon description found");
    }

    let Some(url) = amazon_info.cover_url else {
        return Ok((None, description, publish_date));
    };

    let cover = download_cover(client, &url, images_dir)
        .await
        .map_err(|e| {
            warn!(key = %lookup_key, "Failed to download cover: {}", e);
            e
        })?;

    Ok((Some(cover), description, publish_date))
}

async fn fetch_amazon_cover_with_retry(
    client: &Client,
    lookup_keys: &[String],
    images_dir: &str,
    log_label: &str,
    expected_isbn13: Option<&str>,
) -> (Option<String>, Option<String>, Option<String>) {
    let delays: &[u64] = &[3, 5, 10];
    let mut result: Option<(Option<String>, Option<String>, Option<String>)> = None;
    for key in lookup_keys {
        result = lookup_amazon_cover(client, key, images_dir, expected_isbn13)
            .await
            .ok();
        if result.as_ref().is_some_and(|(c, _, _)| c.is_some()) {
            return result.unwrap();
        }
    }
    for &delay in delays {
        if result.as_ref().is_some_and(|(c, _, _)| c.is_some()) {
            break;
        }
        warn!(key = %log_label, "Amazon cover fetch failed, retrying in {}s", delay);
        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        for key in lookup_keys {
            result = lookup_amazon_cover(client, key, images_dir, expected_isbn13)
                .await
                .ok();
            if result.as_ref().is_some_and(|(c, _, _)| c.is_some()) {
                return result.unwrap();
            }
        }
    }
    result.unwrap_or((None, None, None))
}

pub async fn lookup_amazon_cover_for_jan(
    client: &Client,
    jan: &str,
    images_dir: &str,
) -> Option<String> {
    let (cover, _, _) =
        fetch_amazon_cover_with_retry(client, &[jan.to_string()], images_dir, jan, None).await;
    cover
}

pub async fn lookup_amazon_title_for_jan(client: &Client, jan: &str) -> Option<String> {
    lookup_amazon_info(client, jan, None)
        .await
        .ok()
        .and_then(|info| info.title)
        .filter(|title| !title.trim().is_empty())
}

#[cfg(test)]
fn amazon_search_url(lookup_key: &str) -> String {
    amazon::amazon_search_url(lookup_key).unwrap_or_default()
}
#[cfg(test)]
fn parse_amazon_search_result(html: &str) -> Result<Option<String>, String> {
    Ok(amazon::parse_amazon_search_result(html))
}

fn parse_amazon_detail(html: &str) -> AmazonInfo {
    amazon::parse_amazon_detail(html)
}

#[cfg(test)]
fn amazon_isbn13_matches(info: &AmazonInfo, expected: &str) -> bool {
    info.isbn13.as_deref() == Some(expected)
}

#[cfg(test)]
fn amazon_cover_matches_expected_isbn(info: &AmazonInfo, expected: &str) -> bool {
    amazon::amazon_info_matches_isbn(info, Some(expected))
}

#[cfg(test)]
mod tests {
    use super::{parse_amazon_detail, parse_amazon_search_result};

    #[test]
    fn parses_release_date_from_amazon_detail_bullets() {
        let html = r#"
            <ul id="detailBullets_feature_div">
                <li><span class="a-list-item">
                    <span class="a-text-bold">発売日 :</span>
                    <span>2015/1/26</span>
                </span></li>
                <li><span class="a-list-item">
                    <span class="a-text-bold">ISBN-13 :</span>
                    <span>978-4569823522</span>
                </span></li>
            </ul>
        "#;

        let info = parse_amazon_detail(html);

        assert!(super::amazon_cover_matches_expected_isbn(
            &info,
            "9784569823522"
        ));
        assert!(!super::amazon_cover_matches_expected_isbn(
            &info,
            "9784569823521"
        ));
        assert_eq!(info.publish_date.as_deref(), Some("2015-01-26"));
        assert_eq!(info.isbn13.as_deref(), Some("9784569823522"));
        assert!(super::amazon_isbn13_matches(&info, "9784569823522"));
        assert!(!super::amazon_isbn13_matches(&info, "9784569823521"));
    }

    #[test]
    fn parses_release_date_from_amazon_product_details_table() {
        let html = r#"
            <table id="productDetails_techSpec_section_1">
                <tr><th>発売日</th><td>2015/1/26</td></tr>
            </table>
        "#;

        let info = parse_amazon_detail(html);
        assert!(super::amazon_cover_matches_expected_isbn(
            &info,
            "9784569823522"
        ));

        assert_eq!(info.publish_date.as_deref(), Some("2015-01-26"));
        assert!(!super::amazon_isbn13_matches(&info, "9784569823522"));
    }

    #[test]
    fn parses_isbn10_as_canonical_isbn13() {
        let html = r#"
            <ul id="detailBullets_feature_div">
                <li><span class="a-list-item">
                    <span class="a-text-bold">ISBN-10 :</span>
                    <span>0262033844</span>
                </span></li>
            </ul>
        "#;

        let info = parse_amazon_detail(html);

        assert_eq!(info.isbn13.as_deref(), Some("9780262033848"));
        assert!(super::amazon_isbn13_matches(&info, "9780262033848"));
    }

    #[test]
    fn builds_stripbooks_search_url() {
        assert_eq!(
            super::amazon_search_url("9784041164693"),
            "https://www.amazon.co.jp/s?k=9784041164693&i=stripbooks"
        );
    }

    #[test]
    fn selects_physical_product_over_ebook_variant() {
        let html = r#"
            <div data-component-type="s-search-result" data-asin="B0FX7VSWLJ">
                <a href="/secret-ebook/dp/B0FX7VSWLJ/ref=tmm_kin_swatch_0">Kindle</a>
                <a href="/secret-hardcover/dp/4041164693/ref=sr_1_1">Hardcover</a>
            </div>
        "#;

        assert_eq!(
            parse_amazon_search_result(html).unwrap().as_deref(),
            Some("/secret-hardcover/dp/4041164693/ref=sr_1_1")
        );
    }

    #[test]
    fn does_not_fallback_to_ebook_product() {
        let html = r#"
            <div data-component-type="s-search-result" data-asin="B0FX7VSWLJ">
                <a href="/secret-ebook/dp/B0FX7VSWLJ/ref=tmm_kin_swatch_0">Kindle</a>
            </div>
        "#;

        assert_eq!(parse_amazon_search_result(html).unwrap(), None);
    }

    #[tokio::test]
    #[ignore = "requires live NDL and Amazon access"]
    async fn lookup_isbn_uses_amazon_release_date() {
        let client = reqwest::Client::builder().build().expect("HTTP client");
        let amazon_info =
            super::lookup_amazon_info(&client, "9784569823522", Some("9784569823522"))
                .await
                .expect("Amazon lookup should succeed");
        assert_eq!(amazon_info.isbn13.as_deref(), Some("9784569823522"));
        assert!(super::amazon_isbn13_matches(&amazon_info, "9784569823522"));

        let images_dir =
            std::env::temp_dir().join(format!("dantalian-amazon-isbn-test-{}", std::process::id()));
        std::fs::create_dir_all(&images_dir).expect("create image directory");

        let book = super::lookup_isbn(
            &client,
            "9784569823522",
            images_dir.to_str().expect("temporary path is UTF-8"),
        )
        .await
        .expect("ISBN lookup should succeed")
        .expect("ISBN should resolve to a book");

        let publish_date = book
            .publish_date
            .as_deref()
            .expect("Amazon publication date should resolve");
        assert_eq!(
            crate::external::normalize_publish_date(Some(publish_date)).as_deref(),
            Some(publish_date)
        );
        let cover = book
            .cover_url
            .as_deref()
            .expect("Amazon cover should resolve");
        assert!(images_dir.join(cover).is_file());

        let _ = std::fs::remove_dir_all(images_dir);
    }
}
