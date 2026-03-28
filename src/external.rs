use crate::db::{NewAuthor, NewBook};
use base64::Engine;
use reqwest::Client;
use sha3::{Digest, Sha3_256};
use tracing::{debug, warn};

fn amazon_request(client: &Client, url: &str) -> reqwest::RequestBuilder {
    client.get(url)
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

pub async fn lookup_isbn(client: &Client, isbn: &str, images_dir: &str) -> Result<Option<NewBook>, String> {
    let ndl = lookup_ndl(client, isbn).await?;
    let Some(ndl) = ndl else {
        return Ok(None);
    };

    let cover_url = lookup_amazon_cover(client, isbn, images_dir).await.ok().flatten();

    Ok(Some(NewBook {
        isbn: isbn.to_string(),
        title: ndl.title,
        publisher: ndl.publisher,
        publish_date: ndl.publish_date,
        cover_url,
        description: ndl.description,
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
    let filename = format!("{}.{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash), ext);
    let filepath = std::path::Path::new(images_dir).join(&filename);

    if !filepath.exists() {
        std::fs::write(&filepath, &bytes)
            .map_err(|e| format!("Failed to save cover: {}", e))?;
        debug!(%url, %filename, "Cover saved");
    } else {
        debug!(%url, %filename, "Cover already exists");
    }

    Ok(filename)
}

async fn lookup_amazon_cover(client: &Client, isbn: &str, images_dir: &str) -> Result<Option<String>, String> {
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
        return Ok(None);
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

    let cover_url = tokio::task::spawn_blocking({
        move || parse_amazon_detail_cover(&detail_body)
    })
    .await
    .map_err(|e| format!("Amazon detail parse panicked: {}", e))?;

    match &cover_url {
        Some(url) => debug!(isbn = %isbn, "Amazon cover found: {}", url),
        None => warn!(isbn = %isbn, "No cover image found on Amazon detail page"),
    }

    let Some(url) = cover_url else {
        return Ok(None);
    };

    download_cover(client, &url, images_dir)
        .await
        .map(Some)
        .map_err(|e| {
            warn!(isbn = %isbn, "Failed to download cover: {}", e);
            e
        })
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

    let paper = all_hrefs.iter().find(|h| h.contains("/dp/") && !h.contains("/ebook/dp/"));
    if let Some(href) = paper {
        return Ok(Some(href.to_string()));
    }

    let any_dp = all_hrefs.iter().find(|h| h.contains("/dp/"));
    if let Some(href) = any_dp {
        return Ok(Some(href.to_string()));
    }

    Ok(all_hrefs.first().map(|s| s.to_string()))
}

fn parse_amazon_detail_cover(html: &str) -> Option<String> {
    let document = scraper::Html::parse_document(html);

    if let Ok(selector) = scraper::Selector::parse(r#"img#landingImage"#) {
        if let Some(el) = document.select(&selector).next() {
            if let Some(hires) = el.value().attr("data-old-hires") {
                return Some(hires.to_string());
            }
            if let Some(dynamic) = el.value().attr("data-a-dynamic-image") {
                if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(dynamic) {
                    let max = map.keys().max_by_key(|k| k.len());
                    if let Some(url) = max {
                        return Some(url.clone());
                    }
                }
            }
        }
    }

    if let Ok(selector) = scraper::Selector::parse(r#"img#imgBlkFront"#) {
        if let Some(el) = document.select(&selector).next() {
            if let Some(src) = el.value().attr("src") {
                return Some(src.to_string());
            }
        }
    }

    None
}

async fn lookup_ndl(client: &Client, isbn: &str) -> Result<Option<NdlBookInfo>, String> {
    let raw_query = format!("isbn=\"{}\"", isbn);
    let query = urlencoding::encode(&raw_query);
    let url = format!(
        "https://ndlsearch.ndl.go.jp/api/sru?operation=searchRetrieve&version=1.2&recordSchema=dcndl&onlyBib=true&maximumRecords=1&startRecord=1&recordPacking=xml&query={}",
        query
    );
    let body = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("NDL request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("NDL read failed: {}", e))?;
    parse_ndl_sru(&body)
}

fn parse_ndl_sru(xml: &str) -> Result<Option<NdlBookInfo>, String> {
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
                let prefix = e.name().prefix()
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
                let prefix = e.name().prefix()
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

                    if tag == "creator" && prefix.as_deref() == Some("dcterms") && in_dcterms_creator {
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
                    cur_author_name = Some(text);
                } else if in_dcterms_creator
                    && cur_author_transcription.is_none()
                    && p.len() == 4
                    && p[1] == "creator"
                    && p[2] == "Agent"
                    && p[3] == "transcription"
                {
                    cur_author_transcription = Some(text);
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
