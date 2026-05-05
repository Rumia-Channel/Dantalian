use crate::db::{NewAuthor, NewBook};
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

fn author_name_variants(name: &str) -> Vec<String> {
    let clean = name.trim();
    let mut variants = vec![clean.to_string()];
    let no_space = clean.replace([' ', '　'], "");
    if !no_space.is_empty() && !variants.contains(&no_space) {
        variants.push(no_space);
    }
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.len() == 2 {
        let comma_name = format!("{}, {}", parts[0], parts[1]);
        if !variants.contains(&comma_name) {
            variants.push(comma_name);
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

struct NdlBookInfo {
    title: String,
    publisher: Option<String>,
    publish_date: Option<String>,
    description: Option<String>,
    title_transcription: Option<String>,
    series_title: Option<String>,
    series_title_transcription: Option<String>,
    alternative: Option<String>,
    alternative_transcription: Option<String>,
    volume: Option<String>,
    volume_transcription: Option<String>,
    price: Option<String>,
    extent: Option<String>,
    jpno: Option<String>,
    ndl_url: Option<String>,
    authors: Vec<NewAuthor>,
}

pub async fn lookup_isbn(
    client: &Client,
    isbn: &str,
    images_dir: &str,
) -> Result<Option<NewBook>, String> {
    let isbn_variants = isbn_lookup_variants(isbn);
    let Some(mut ndl) = lookup_ndl(client, &isbn_variants).await? else {
        return Ok(None);
    };
    enrich_author_ndl_ids(client, &mut ndl.authors).await;

    let (cover_url, amazon_description) = {
        let delays: &[u64] = &[3, 5, 10];
        let mut result = None;
        for lookup_isbn in &isbn_variants {
            result = lookup_amazon_cover(client, lookup_isbn, images_dir)
                .await
                .ok();
            if result.as_ref().is_some_and(|(c, _)| c.is_some()) {
                break;
            }
        }
        for &delay in delays {
            if result.as_ref().is_some_and(|(c, _)| c.is_some()) {
                break;
            }
            warn!(isbn = %isbn, "Amazon cover fetch failed, retrying in {}s", delay);
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            for lookup_isbn in &isbn_variants {
                result = lookup_amazon_cover(client, lookup_isbn, images_dir)
                    .await
                    .ok();
                if result.as_ref().is_some_and(|(c, _)| c.is_some()) {
                    break;
                }
            }
        }
        result.unwrap_or((None, None))
    };

    let description = amazon_description.or(ndl.description);

    Ok(Some(NewBook {
        isbn: Some(isbn.to_string()),
        isdn: None,
        title: ndl.title,
        publisher: ndl.publisher,
        publish_date: ndl.publish_date,
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

async fn lookup_amazon_cover(
    client: &Client,
    isbn: &str,
    images_dir: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let search_url = format!("https://www.amazon.co.jp/s?k={}", isbn);
    debug!(isbn = %isbn, "Amazon search: {}", search_url);
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
        warn!(isbn = %isbn, "No product link found in Amazon search results");
        return Ok((None, None));
    };

    let detail_url = if href.starts_with("http") {
        href
    } else {
        format!("https://www.amazon.co.jp{}", href)
    };
    debug!(isbn = %isbn, "Amazon detail: {}", detail_url);

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
        debug!(isbn = %isbn, "Amazon black-curtain detected, confirming age eligibility");
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

    match &amazon_info.cover_url {
        Some(url) => debug!(isbn = %isbn, "Amazon cover found: {}", url),
        None => warn!(isbn = %isbn, "No cover image found on Amazon detail page"),
    }
    if amazon_info.description.is_some() {
        debug!(isbn = %isbn, "Amazon description found");
    }

    let Some(url) = amazon_info.cover_url else {
        return Ok((None, amazon_info.description));
    };

    let cover = download_cover(client, &url, images_dir)
        .await
        .map_err(|e| {
            warn!(isbn = %isbn, "Failed to download cover: {}", e);
            e
        })?;

    Ok((Some(cover), amazon_info.description))
}

fn parse_amazon_search_result(html: &str) -> Result<Option<String>, String> {
    let document = scraper::Html::parse_document(html);

    let selector = scraper::Selector::parse(r#"[cel_widget_id^="MAIN-SEARCH_RESULTS-"]"#)
        .map_err(|e| format!("Amazon selector parse failed: {}", e))?;

    let a_selector = scraper::Selector::parse("a[href]").unwrap();

    let widget = match document.select(&selector).next() {
        Some(el) => el,
        None => return Ok(None),
    };

    let all_hrefs: Vec<&str> = widget
        .select(&a_selector)
        .filter_map(|a| a.value().attr("href"))
        .collect();

    let paper = all_hrefs
        .iter()
        .find(|h| h.contains("/dp/") && !h.contains("/ebook/dp/"));
    if let Some(href) = paper {
        return Ok(Some(href.to_string()));
    }

    let any_dp = all_hrefs.iter().find(|h| h.contains("/dp/"));
    if let Some(href) = any_dp {
        return Ok(Some(href.to_string()));
    }

    Ok(all_hrefs.first().map(|s| s.to_string()))
}

struct AmazonInfo {
    cover_url: Option<String>,
    description: Option<String>,
}

fn parse_amazon_detail(html: &str) -> AmazonInfo {
    let document = scraper::Html::parse_document(html);

    let cover_url = if let Ok(selector) = scraper::Selector::parse(r#"img#landingImage"#) {
        document.select(&selector).next().and_then(|el| {
            el.value().attr("data-old-hires").map(|s| s.to_string()).or_else(|| {
                el.value().attr("data-a-dynamic-image").and_then(|dynamic| {
                    serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(dynamic)
                        .ok()
                        .and_then(|map| map.keys().max_by_key(|k| k.len()).cloned())
                })
            })
        })
    } else {
        None
    };

    let cover_url = cover_url.or_else(|| {
        scraper::Selector::parse(r#"img#imgBlkFront"#)
            .ok()
            .and_then(|selector| {
                document
                    .select(&selector)
                    .next()
                    .and_then(|el| el.value().attr("src").map(|s| s.to_string()))
            })
    });

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

    AmazonInfo {
        cover_url,
        description,
    }
}

async fn lookup_ndl(
    client: &Client,
    isbn_variants: &[String],
) -> Result<Option<NdlBookInfo>, String> {
    let mut candidates = Vec::new();

    for isbn in isbn_variants {
        let raw_query = format!("isbn=\"{}\"", isbn);
        let query = urlencoding::encode(&raw_query);
        let url = format!(
            "https://ndlsearch.ndl.go.jp/api/sru?operation=searchRetrieve&version=1.2&recordSchema=dcndl&onlyBib=true&maximumRecords=10&startRecord=1&recordPacking=xml&query={}",
            query
        );
        debug!(%isbn, "NDL search: {}", url);
        let body = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("NDL request failed: {}", e))?
            .text()
            .await
            .map_err(|e| format!("NDL read failed: {}", e))?;
        candidates.extend(parse_ndl_sru_candidates(&body)?);
    }

    candidates.sort_by_key(ndl_score);
    Ok(candidates.pop())
}

fn ndl_score(info: &NdlBookInfo) -> i32 {
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

fn parse_ndl_sru_candidates(xml: &str) -> Result<Vec<NdlBookInfo>, String> {
    let mut candidates = Vec::new();
    let mut offset = 0;

    while let Some(start_rel) = xml[offset..].find("<recordData") {
        let start = offset + start_rel;
        let Some(open_end_rel) = xml[start..].find('>') else {
            break;
        };
        let content_start = start + open_end_rel + 1;
        let Some(end_rel) = xml[content_start..].find("</recordData>") else {
            break;
        };
        let end = content_start + end_rel + "</recordData>".len();
        let chunk = &xml[start..end];
        if let Some(info) = parse_ndl_sru_record(chunk)? {
            candidates.push(info);
        }
        offset = end;
    }

    if candidates.is_empty() {
        if let Some(info) = parse_ndl_sru_record(xml)? {
            candidates.push(info);
        }
    }

    Ok(candidates)
}

fn parse_ndl_sru_record(xml: &str) -> Result<Option<NdlBookInfo>, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut in_record_data = false;
    let mut in_main_resource = false;
    let mut path: Vec<String> = Vec::new();

    let mut title: Option<String> = None;
    let mut publisher: Option<String> = None;
    let mut pub_date: Option<String> = None;
    let mut description: Option<String> = None;
    let mut title_transcription: Option<String> = None;
    let mut series_title: Option<String> = None;
    let mut series_title_transcription: Option<String> = None;
    let mut alternative: Option<String> = None;
    let mut alternative_transcription: Option<String> = None;
    let mut volume: Option<String> = None;
    let mut volume_transcription: Option<String> = None;
    let mut price: Option<String> = None;
    let mut extent: Option<String> = None;
    let mut jpno: Option<String> = None;
    let mut ndl_url: Option<String> = None;
    let mut authors: Vec<NewAuthor> = Vec::new();
    let mut cur_author_ndl_id: Option<String> = None;
    let mut cur_author_name: Option<String> = None;
    let mut cur_author_transcription: Option<String> = None;
    let mut in_dcterms_creator = false;

    let mut buf = Vec::new();

    use quick_xml::events::Event;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().local_name().as_ref()).to_string();
                let prefix = e
                    .name()
                    .prefix()
                    .map(|p| String::from_utf8_lossy(p.as_ref()).to_string());

                if tag == "recordData" {
                    in_record_data = true;
                    path.clear();
                } else if in_record_data && !in_main_resource && tag == "BibResource" {
                    for attr in e.attributes().flatten() {
                        if attr.key.local_name().as_ref() == b"about"
                            && attr.value.windows(9).any(|w| w == b"#material")
                        {
                            in_main_resource = true;
                            ndl_url = Some(String::from_utf8_lossy(&attr.value).to_string());
                            path.clear();
                            break;
                        }
                    }
                    if in_main_resource {
                        path.push(tag);
                    }
                } else if in_main_resource {
                    let is_creator = tag == "creator" && prefix.as_deref() == Some("dcterms");
                    path.push(tag.clone());

                    if is_creator {
                        in_dcterms_creator = true;
                        cur_author_ndl_id = None;
                        cur_author_name = None;
                        cur_author_transcription = None;
                    }

                    if tag == "Agent" && in_dcterms_creator {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"about" {
                                let full = String::from_utf8_lossy(&attr.value);
                                let id = full.rsplit('/').next().map(|s| s.to_string());
                                cur_author_ndl_id = id;
                                break;
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().local_name().as_ref()).to_string();
                let prefix = e
                    .name()
                    .prefix()
                    .map(|p| String::from_utf8_lossy(p.as_ref()).to_string());

                if in_record_data && tag == "recordData" {
                    break;
                }
                if in_main_resource && tag == "BibResource" && path.len() == 1 {
                    in_main_resource = false;
                }
                if in_main_resource {
                    if path.last().map(|s| s.as_str()) == Some(&tag) {
                        path.pop();
                    }

                    if tag == "creator"
                        && prefix.as_deref() == Some("dcterms")
                        && in_dcterms_creator
                    {
                        in_dcterms_creator = false;
                        if let Some(name) = cur_author_name.take() {
                            authors.push(NewAuthor {
                                ndl_id: cur_author_ndl_id.take(),
                                name,
                                transcription: cur_author_transcription.take(),
                            });
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) if in_main_resource => {
                let text = e.decode().unwrap_or_default().to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }

                let p = &path;

                if title.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "title"
                    && p[2] == "Description"
                    && p[3] == "value"
                {
                    title = Some(text);
                } else if in_dcterms_creator
                    && cur_author_name.is_none()
                    && p.len() == 4
                    && p[1] == "creator"
                    && p[2] == "Agent"
                    && p[3] == "name"
                {
                    cur_author_name = Some(text.replace(',', "").replace('，', ""));
                } else if in_dcterms_creator
                    && cur_author_transcription.is_none()
                    && p.len() == 4
                    && p[1] == "creator"
                    && p[2] == "Agent"
                    && p[3] == "transcription"
                {
                    cur_author_transcription = Some(text.replace(',', "").replace('，', ""));
                } else if publisher.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "publisher"
                    && p[2] == "Agent"
                    && p[3] == "name"
                {
                    publisher = Some(text);
                } else if pub_date.is_none()
                    && p.len() == 2
                    && p[0] == "BibResource"
                    && p[1] == "date"
                {
                    pub_date = Some(text);
                } else if title_transcription.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "title"
                    && p[2] == "Description"
                    && p[3] == "transcription"
                {
                    title_transcription = Some(text);
                } else if series_title_transcription.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "seriesTitle"
                    && p[2] == "Description"
                    && p[3] == "transcription"
                {
                    series_title_transcription = Some(text);
                } else if series_title.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "seriesTitle"
                    && p[2] == "Description"
                    && p[3] == "value"
                {
                    series_title = Some(text);
                } else if price.is_none()
                    && p.len() == 2
                    && p[0] == "BibResource"
                    && p[1] == "price"
                {
                    price = Some(text);
                } else if extent.is_none()
                    && p.len() == 2
                    && p[0] == "BibResource"
                    && p[1] == "extent"
                {
                    extent = Some(text);
                } else if alternative.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "alternative"
                    && p[2] == "Description"
                    && p[3] == "value"
                {
                    alternative = Some(text);
                } else if alternative_transcription.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "alternative"
                    && p[2] == "Description"
                    && p[3] == "transcription"
                {
                    alternative_transcription = Some(text);
                } else if volume.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "volume"
                    && p[2] == "Description"
                    && p[3] == "value"
                {
                    volume = Some(text);
                } else if volume_transcription.is_none()
                    && p.len() == 4
                    && p[0] == "BibResource"
                    && p[1] == "volume"
                    && p[2] == "Description"
                    && p[3] == "transcription"
                {
                    volume_transcription = Some(text);
                } else if jpno.is_none()
                    && p.len() == 2
                    && p[0] == "BibResource"
                    && p[1] == "identifier"
                    && text.chars().all(|c| c.is_ascii_digit())
                {
                    jpno = Some(text);
                } else if description.is_none()
                    && p.len() == 2
                    && p[0] == "BibResource"
                    && p[1] == "description"
                    && !text.starts_with("表現種別")
                    && !text.starts_with("機器種別")
                    && !text.starts_with("キャリア種別")
                {
                    description = Some(text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    let title = match title {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(None),
    };

    Ok(Some(NdlBookInfo {
        title,
        publisher,
        publish_date: pub_date,
        description,
        title_transcription,
        series_title,
        series_title_transcription,
        alternative,
        alternative_transcription,
        volume,
        volume_transcription,
        price,
        extent,
        jpno,
        ndl_url,
        authors,
    }))
}

async fn enrich_author_ndl_ids(client: &Client, authors: &mut [NewAuthor]) {
    for author in authors {
        if author.ndl_id.is_some() {
            continue;
        }
        match lookup_ndl_author_id(client, &author.name).await {
            Ok(Some(id)) => author.ndl_id = Some(id),
            Ok(None) => {}
            Err(e) => warn!(author = %author.name, "NDL authority lookup failed: {}", e),
        }
    }
}

async fn lookup_ndl_author_id(client: &Client, name: &str) -> Result<Option<String>, String> {
    let variants = author_name_variants(name);
    if variants.is_empty() {
        return Ok(None);
    }

    let union_patterns = variants
        .iter()
        .map(|v| {
            let escaped = v.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                r#"{{ ?auth foaf:primaryTopic ?entity . ?entity foaf:name "{escaped}" . }}
  UNION {{ ?auth foaf:primaryTopic ?entity ; rdfs:label "{escaped}" . }}"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n  UNION ");
    let query = format!(
        r#"PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT ?entity WHERE {{
  {}
}}
LIMIT 1"#,
        union_patterns
    );
    let url = format!(
        "https://id.ndl.go.jp/auth/ndla?output=json&query={}",
        urlencoding::encode(&query)
    );
    let value: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("NDL authority request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("NDL authority response parse failed: {}", e))?;

    let entity = value
        .pointer("/results/bindings/0/entity/value")
        .and_then(|v| v.as_str());
    Ok(entity.and_then(|uri| uri.rsplit('/').next().map(|id| id.to_string())))
}

struct IsdnBookInfo {
    title: String,
    publisher: Option<String>,
    publish_date: Option<String>,
    description: Option<String>,
    title_transcription: Option<String>,
    price: Option<String>,
    extent: Option<String>,
    cover_url: Option<String>,
    region: Option<String>,
    class: Option<String>,
    isdn_type: Option<String>,
    rating_gender: Option<String>,
    rating_age: Option<String>,
    genre_code: Option<String>,
    genre_name: Option<String>,
    genre_user: Option<String>,
    c_code: Option<String>,
    author: Option<String>,
    shape: Option<String>,
    contents: Option<String>,
    barcode2: Option<String>,
    sample_image_url: Option<String>,
    useroption: Option<String>,
    external_links: Option<String>,
}

async fn parse_isdn_xml(xml: &str) -> Result<Option<IsdnBookInfo>, String> {
    let result = tokio::task::spawn_blocking({
        let xml = xml.to_string();
        move || {
            let document = scraper::Html::parse_document(&xml);

            fn text_val(el: &scraper::ElementRef, tag: &str) -> Option<String> {
                let sel = scraper::Selector::parse(tag).unwrap();
                el.select(&sel)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .filter(|s| !s.is_empty())
            }

            let item_sel = scraper::Selector::parse("item").map_err(|e| e.to_string())?;

            let item = match document.select(&item_sel).next() {
                Some(el) => el,
                None => return Ok(None),
            };

            let title = text_val(&item, "product-name").unwrap_or_default();
            if title.is_empty() {
                return Ok(None);
            }

            let title_transcription = text_val(&item, "product-yomi");
            let publisher = text_val(&item, "publisher-name");
            let publish_date = text_val(&item, "issue-date");
            let description = text_val(&item, "product-comment");

            let price_val = text_val(&item, "price");
            let price_unit = text_val(&item, "price-unit");
            let price = match (price_val, price_unit) {
                (Some(val), Some(unit)) => Some(format!("{} {}", val, unit)),
                (Some(val), None) => Some(val),
                _ => None,
            };

            let style = text_val(&item, "product-style");
            let size = text_val(&item, "product-size");
            let capacity = text_val(&item, "product-capacity");
            let capacity_unit = text_val(&item, "product-capacity-unit");
            let extent = match (style, size, capacity, capacity_unit) {
                (Some(s), Some(sz), Some(c), Some(cu)) => {
                    Some(format!("{}, {}, {}{}", s, sz, c, cu))
                }
                (Some(s), Some(sz), None, None) => Some(format!("{}, {}", s, sz)),
                (Some(s), None, None, None) => Some(s),
                (None, Some(sz), None, None) => Some(sz),
                _ => None,
            };

            let cover_url = text_val(&item, "sample-image-uri");
            let region = text_val(&item, "region");
            let class = text_val(&item, "class");
            let isdn_type = text_val(&item, "type");
            let rating_gender = text_val(&item, "rating_gender");
            let rating_age = text_val(&item, "rating_age");
            let genre_code = text_val(&item, "genre-code");
            let genre_name = text_val(&item, "genre-name");
            let genre_user = text_val(&item, "genre-user");
            let c_code = text_val(&item, "c-code");
            let author = text_val(&item, "author");
            let shape = text_val(&item, "shape");
            let contents = text_val(&item, "contents");
            let barcode2 = text_val(&item, "barcode2");

            let sample_image_url = cover_url.clone();

            let useroption = {
                let sel = scraper::Selector::parse("useroption").unwrap();
                let items: Vec<(String, String)> = item
                    .select(&sel)
                    .filter_map(|uo| {
                        let prop_sel = scraper::Selector::parse("property").unwrap();
                        let val_sel = scraper::Selector::parse("value").unwrap();
                        let prop = uo
                            .select(&prop_sel)
                            .next()
                            .map(|e| e.text().collect::<String>())
                            .filter(|s| !s.is_empty())?;
                        let val = uo
                            .select(&val_sel)
                            .next()
                            .map(|e| e.text().collect::<String>())
                            .filter(|s| !s.is_empty())?;
                        Some((prop, val))
                    })
                    .collect();
                if items.is_empty() {
                    None
                } else {
                    Some(serde_json::to_string(&items).unwrap())
                }
            };

            let external_links = {
                let sel = scraper::Selector::parse("external-link").unwrap();
                let links: Vec<(String, String)> = item
                    .select(&sel)
                    .filter_map(|link| {
                        let title_sel = scraper::Selector::parse("title").unwrap();
                        let uri_sel = scraper::Selector::parse("uri").unwrap();
                        let t = link
                            .select(&title_sel)
                            .next()
                            .map(|e| e.text().collect::<String>())
                            .filter(|s| !s.is_empty())?;
                        let u = link
                            .select(&uri_sel)
                            .next()
                            .map(|e| e.text().collect::<String>())
                            .filter(|s| !s.is_empty())?;
                        Some((t, u))
                    })
                    .collect();
                if links.is_empty() {
                    None
                } else {
                    Some(serde_json::to_string(&links).unwrap())
                }
            };

            Ok(Some(IsdnBookInfo {
                title,
                publisher,
                publish_date,
                description,
                title_transcription,
                price,
                extent,
                cover_url,
                region,
                class,
                isdn_type,
                rating_gender,
                rating_age,
                genre_code,
                genre_name,
                genre_user,
                c_code,
                author,
                shape,
                contents,
                barcode2,
                sample_image_url,
                useroption,
                external_links,
            }))
        }
    })
    .await
    .map_err(|e: tokio::task::JoinError| e.to_string())?;

    result
}

pub async fn lookup_isdn(client: &Client, isdn: &str) -> Result<Option<NewBook>, String> {
    let clean: String = isdn.chars().filter(|c| c.is_ascii_digit()).collect();
    if clean.len() != 13 {
        return Err("ISDN must be 13 digits".to_string());
    }

    let url = format!("https://isdn.jp/xml/{}", clean);
    debug!(%url, "Fetching ISDN XML");

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ISDN request failed: {}", e))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let xml = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read ISDN response: {}", e))?;

    let info = parse_isdn_xml(&xml)
        .await?
        .ok_or("No item found in ISDN response")?;

    let mut description = info.description;
    if description.is_none() {
        if let Some(ref ext) = info.extent {
            description = Some(ext.clone());
        }
    }

    Ok(Some(NewBook {
        isbn: None,
        isdn: Some(clean),
        title: info.title,
        publisher: info.publisher,
        publish_date: info.publish_date,
        cover_url: info.cover_url,
        description,
        title_transcription: info.title_transcription,
        series_title: None,
        series_title_transcription: None,
        alternative: None,
        alternative_transcription: None,
        volume: None,
        volume_transcription: None,
        price: info.price,
        extent: info.extent,
        jpno: None,
        ndl_url: None,
        authors: Vec::new(),
        isdn_region: info.region,
        isdn_class: info.class,
        isdn_type: info.isdn_type,
        isdn_rating_gender: info.rating_gender,
        isdn_rating_age: info.rating_age,
        isdn_genre_code: info.genre_code,
        isdn_genre_name: info.genre_name,
        isdn_genre_user: info.genre_user,
        isdn_c_code: info.c_code,
        isdn_author: info.author,
        isdn_shape: info.shape,
        isdn_contents: info.contents,
        isdn_barcode2: info.barcode2,
        isdn_sample_image_url: info.sample_image_url,
        isdn_useroption: info.useroption,
        isdn_external_links: info.external_links,
    }))
}
