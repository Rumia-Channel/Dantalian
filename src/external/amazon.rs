use crate::db::NewBook;
use base64::Engine;
use reqwest::Client;
use sha3::{Digest, Sha3_256};
use tracing::{debug, warn};

fn isbn13_to_isbn10(isbn13: &str) -> Option<String> {
    let digits: Vec<u8> = isbn13
        .chars()
        .filter_map(|c| c.to_digit(10).map(|d| d as u8))
        .collect();
    if digits.len() != 13
        || (digits[0] != 9 || digits[1] != 7 || (digits[2] != 8 && digits[2] != 9))
    {
        return None;
    }
    let body = &digits[3..12];
    let mut sum: u32 = 0;
    for (i, &d) in body.iter().enumerate() {
        sum += (d as u32) * ((i + 1) as u32);
    }
    let check = (11 - (sum % 11)) % 11;
    let check_char = if check == 10 {
        'X'
    } else {
        char::from_digit(check as u32, 10)?
    };
    let s: String = body
        .iter()
        .map(|d| char::from_digit(*d as u32, 10).unwrap())
        .collect();
    Some(format!("{}{}", s, check_char))
}

fn isbn10_to_isbn13(isbn10: &str) -> Option<String> {
    let clean = isbn10.trim().replace(['-', ' '], "").to_uppercase();
    if clean.len() != 10 {
        return None;
    }
    let body = &clean[..9];
    if !body.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut digits = format!("978{}", body);
    let sum: u32 = digits
        .chars()
        .enumerate()
        .map(|(i, c)| c.to_digit(10).unwrap() * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    let check = (10 - (sum % 10)) % 10;
    digits.push(char::from_digit(check, 10)?);
    Some(digits)
}

fn isbn_lookup_variants(isbn: &str) -> Vec<String> {
    let clean = isbn.trim().replace(['-', ' '], "").to_uppercase();
    let mut variants = vec![clean.clone()];
    let converted = match clean.len() {
        10 => isbn10_to_isbn13(&clean),
        13 => isbn13_to_isbn10(&clean),
        _ => None,
    };
    if let Some(other) = converted {
        if !variants.contains(&other) {
            variants.push(other);
        }
    }
    variants
}

fn amazon_request(client: &Client, url: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("User-Agent", ua_generator::ua::spoof_ua())
        .header("Accept-Language", "ja-JP,ja;q=0.9,en-US;q=0.8,en;q=0.7")
}

fn is_valid_cover_url(u: &str) -> bool {
    !u.is_empty()
        && (u.starts_with("http://") || u.starts_with("https://"))
        && !u.starts_with("data:")
}

pub async fn lookup_isbn(
    client: &Client,
    isbn: &str,
    images_dir: &str,
) -> Result<Option<NewBook>, String> {
    let isbn_variants = isbn_lookup_variants(isbn);
    let Some(mut ndl) = super::ndl::lookup_ndl(client, &isbn_variants).await? else {
        return Ok(None);
    };
    super::ndl::enrich_author_ndl_ids(client, &mut ndl.authors).await;

    let (cover_url, amazon_description, amazon_publish_date) =
        fetch_amazon_cover_with_retry(client, &isbn_variants, images_dir, isbn).await;

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

async fn lookup_amazon_info(client: &Client, lookup_key: &str) -> Result<AmazonInfo, String> {
    let search_url = format!("https://www.amazon.co.jp/s?k={}", lookup_key);
    debug!(key = %lookup_key, "Amazon search: {}", search_url);
    let search_body = amazon_request(client, &search_url)
        .send()
        .await
        .map_err(|e| format!("Amazon request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Amazon read failed: {}", e))?;

    let product_url = tokio::task::spawn_blocking({
        let body = search_body;
        move || parse_amazon_search_result(&body)
    })
    .await
    .map_err(|e| format!("Amazon search parse panicked: {}", e))?
    .ok()
    .flatten();

    let Some(href) = product_url else {
        warn!(key = %lookup_key, "No product link found in Amazon search results");
        return Ok(AmazonInfo {
            title: None,
            cover_url: None,
            description: None,
            publish_date: None,
        });
    };

    let detail_url = if href.starts_with("http") {
        href
    } else {
        format!("https://www.amazon.co.jp{}", href)
    };
    debug!(key = %lookup_key, "Amazon detail: {}", detail_url);

    let detail_body = amazon_request(client, &detail_url)
        .send()
        .await
        .map_err(|e| format!("Amazon detail request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Amazon detail read failed: {}", e))?;

    let has_black_curtain = tokio::task::spawn_blocking({
        let body = detail_body.clone();
        move || body.contains("black-curtain-verification")
    })
    .await
    .map_err(|e| format!("Black curtain check panicked: {}", e))?;

    let detail_body = if has_black_curtain {
        debug!(key = %lookup_key, "Amazon black-curtain detected, confirming age eligibility");
        let detail_path = detail_url
            .find("/dp/")
            .map(|i| &detail_url[i..])
            .unwrap_or(&detail_url);
        let eligibility_url = format!(
            "https://www.amazon.co.jp/black-curtain/save-eligibility/black-curtain?returnUrl={}",
            urlencoding::encode(detail_path)
        );
        amazon_request(client, &eligibility_url)
            .send()
            .await
            .map_err(|e| format!("Amazon age eligibility request failed: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Amazon age eligibility read failed: {}", e))?;

        amazon_request(client, &detail_url)
            .send()
            .await
            .map_err(|e| format!("Amazon detail retry failed: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Amazon detail retry read failed: {}", e))?
    } else {
        detail_body
    };

    let amazon_info = tokio::task::spawn_blocking(move || parse_amazon_detail(&detail_body))
        .await
        .map_err(|e| format!("Amazon detail parse panicked: {}", e))?;

    Ok(amazon_info)
}

async fn lookup_amazon_cover(
    client: &Client,
    lookup_key: &str,
    images_dir: &str,
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    let amazon_info = lookup_amazon_info(client, lookup_key).await?;
    match &amazon_info.cover_url {
        Some(url) => debug!(key = %lookup_key, "Amazon cover found: {}", url),
        None => warn!(key = %lookup_key, "No cover image found on Amazon detail page"),
    }
    if amazon_info.description.is_some() {
        debug!(key = %lookup_key, "Amazon description found");
    }

    let Some(url) = amazon_info.cover_url else {
        return Ok((None, amazon_info.description, amazon_info.publish_date));
    };

    let cover = download_cover(client, &url, images_dir)
        .await
        .map_err(|e| {
            warn!(key = %lookup_key, "Failed to download cover: {}", e);
            e
        })?;

    Ok((
        Some(cover),
        amazon_info.description,
        amazon_info.publish_date,
    ))
}

async fn fetch_amazon_cover_with_retry(
    client: &Client,
    lookup_keys: &[String],
    images_dir: &str,
    log_label: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let delays: &[u64] = &[3, 5, 10];
    let mut result: Option<(Option<String>, Option<String>, Option<String>)> = None;
    for key in lookup_keys {
        result = lookup_amazon_cover(client, key, images_dir).await.ok();
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
            result = lookup_amazon_cover(client, key, images_dir).await.ok();
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
        fetch_amazon_cover_with_retry(client, &[jan.to_string()], images_dir, jan).await;
    cover
}

pub async fn lookup_amazon_title_for_jan(client: &Client, jan: &str) -> Option<String> {
    lookup_amazon_info(client, jan)
        .await
        .ok()
        .and_then(|info| info.title)
        .filter(|title| !title.trim().is_empty())
}

fn parse_amazon_search_result(html: &str) -> Result<Option<String>, String> {
    let document = scraper::Html::parse_document(html);

    let card_selector = scraper::Selector::parse(r#"[data-component-type="s-search-result"]"#)
        .map_err(|e| format!("Amazon selector parse failed: {}", e))?;
    let a_selector = scraper::Selector::parse("a[href]").unwrap();

    if let Some(card) = document.select(&card_selector).find(|el| {
        el.value()
            .attr("data-asin")
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }) {
        let hrefs: Vec<&str> = card
            .select(&a_selector)
            .filter_map(|a| a.value().attr("href"))
            .collect();

        if let Some(href) = hrefs
            .iter()
            .find(|h| h.contains("/dp/") && !h.contains("/ebook/dp/"))
        {
            return Ok(Some((*href).to_string()));
        }
        if let Some(href) = hrefs.iter().find(|h| h.contains("/dp/")) {
            return Ok(Some((*href).to_string()));
        }
    }

    let main_slot = scraper::Selector::parse("div.s-result-list, div.s-main-slot")
        .ok()
        .and_then(|sel| document.select(&sel).next());

    if let Some(slot) = main_slot {
        let hrefs: Vec<&str> = slot
            .select(&a_selector)
            .filter_map(|a| a.value().attr("href"))
            .collect();

        if let Some(href) = hrefs
            .iter()
            .find(|h| h.contains("/dp/") && !h.contains("/ebook/dp/"))
        {
            return Ok(Some((*href).to_string()));
        }
        if let Some(href) = hrefs.iter().find(|h| h.contains("/dp/")) {
            return Ok(Some((*href).to_string()));
        }
    }

    let old_selector = scraper::Selector::parse(r#"[cel_widget_id^="MAIN-SEARCH_RESULTS-"]"#)
        .map_err(|e| format!("Amazon selector parse failed: {}", e))?;

    if let Some(widget) = document.select(&old_selector).next() {
        let hrefs: Vec<&str> = widget
            .select(&a_selector)
            .filter_map(|a| a.value().attr("href"))
            .collect();
        if let Some(href) = hrefs
            .iter()
            .find(|h| h.contains("/dp/") && !h.contains("/ebook/dp/"))
        {
            return Ok(Some((*href).to_string()));
        }
        if let Some(href) = hrefs.iter().find(|h| h.contains("/dp/")) {
            return Ok(Some((*href).to_string()));
        }
    }

    Ok(None)
}

struct AmazonInfo {
    title: Option<String>,
    cover_url: Option<String>,
    description: Option<String>,
    publish_date: Option<String>,
}

fn parse_amazon_detail(html: &str) -> AmazonInfo {
    let document = scraper::Html::parse_document(html);

    let title = ["#productTitle", "#title"]
        .iter()
        .filter_map(|selector| scraper::Selector::parse(selector).ok())
        .find_map(|selector| {
            document
                .select(&selector)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            scraper::Selector::parse(r#"meta[property="og:title"]"#)
                .ok()
                .and_then(|selector| {
                    document
                        .select(&selector)
                        .next()
                        .and_then(|el| el.value().attr("content"))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
        });

    let cover_url = if let Ok(selector) = scraper::Selector::parse(r#"img#landingImage"#) {
        document.select(&selector).next().and_then(|el| {
            let hires = el
                .value()
                .attr("data-old-hires")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            hires
                .or_else(|| {
                    el.value()
                        .attr("data-a-dynamic-image")
                        .and_then(|dynamic| {
                            serde_json::from_str::<
                                std::collections::HashMap<String, serde_json::Value>,
                            >(dynamic)
                            .ok()
                        })
                        .and_then(|map| {
                            map.keys()
                                .filter(|k| !k.is_empty())
                                .max_by_key(|k| k.len())
                                .cloned()
                        })
                })
                .or_else(|| {
                    el.value()
                        .attr("src")
                        .map(str::trim)
                        .filter(|s| !s.is_empty() && !s.starts_with("data:"))
                        .map(|s| s.to_string())
                })
        })
    } else {
        None
    };

    let cover_url = cover_url.filter(|u| !u.is_empty()).or_else(|| {
        scraper::Selector::parse(r#"img#imgBlkFront"#)
            .ok()
            .and_then(|selector| {
                document
                    .select(&selector)
                    .next()
                    .and_then(|el| el.value().attr("src").map(|s| s.to_string()))
            })
    });

    let cover_url = cover_url.filter(|u| is_valid_cover_url(u));

    let description =
        scraper::Selector::parse(r#"div#bookDescription_feature_div div.a-expander-content span"#)
            .ok()
            .and_then(|selector| {
                document
                    .select(&selector)
                    .next()
                    .map(|el| {
                        let html = el.inner_html();
                        let text = html
                            .replace("<br>", "\n")
                            .replace("<br/>", "\n")
                            .replace("<br />", "\n")
                            .replace("<BR>", "\n")
                            .replace("<BR/>", "\n")
                            .replace("<BR />", "\n");
                        let trimmed = text.trim().to_string();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed)
                        }
                    })
                    .flatten()
            });

    let publish_date = parse_amazon_publish_date(&document);

    AmazonInfo {
        title,
        cover_url,
        description,
        publish_date,
    }
}

fn parse_amazon_publish_date(document: &scraper::Html) -> Option<String> {
    const SELECTORS: &[&str] = &[
        "#detailBullets_feature_div li",
        "#detailBulletsWrapper_feature_div li",
        "#productDetails_techSpec_section_1 tr",
        "#productDetails_detailBullets_sections1 tr",
        "#productDetailsTable tr",
    ];

    for selector in SELECTORS {
        let Ok(selector) = scraper::Selector::parse(selector) else {
            continue;
        };
        for element in document.select(&selector) {
            let text = element.text().collect::<String>();
            if !text.contains("発売日") && !text.contains("出版日") {
                continue;
            }

            for label in ["発売日", "出版日"] {
                let Some((_, value)) = text.split_once(label) else {
                    continue;
                };
                if let Some(date) = value
                    .split_whitespace()
                    .filter_map(|part| {
                        let trimmed = part.trim_matches(|ch: char| {
                            matches!(
                                ch,
                                ':' | '：' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}'
                            )
                        });
                        crate::external::normalize_publish_date(Some(trimmed))
                    })
                    .next()
                {
                    return Some(date);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_amazon_detail;

    #[test]
    fn parses_release_date_from_amazon_detail_bullets() {
        let html = r#"
            <ul id="detailBullets_feature_div">
                <li><span class="a-list-item">
                    <span class="a-text-bold">発売日 :</span>
                    <span>2015/1/26</span>
                </span></li>
            </ul>
        "#;

        let info = parse_amazon_detail(html);

        assert_eq!(info.publish_date.as_deref(), Some("2015-01-26"));
    }

    #[test]
    fn parses_release_date_from_amazon_product_details_table() {
        let html = r#"
            <table id="productDetails_techSpec_section_1">
                <tr><th>発売日</th><td>2015/1/26</td></tr>
            </table>
        "#;

        let info = parse_amazon_detail(html);

        assert_eq!(info.publish_date.as_deref(), Some("2015-01-26"));
    }

    #[tokio::test]
    #[ignore = "requires live NDL and Amazon access"]
    async fn lookup_isbn_uses_amazon_release_date() {
        let client = reqwest::Client::builder().build().expect("HTTP client");
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

        assert_eq!(book.publish_date.as_deref(), Some("2015-01-26"));

        let _ = std::fs::remove_dir_all(images_dir);
    }
}
