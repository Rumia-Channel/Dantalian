use crate::db::NewBook;
use reqwest::Client;
use tracing::{debug, warn};

fn amazon_request(client: &Client, url: &str) -> reqwest::RequestBuilder {
    client.get(url)
        .header("User-Agent", ua_generator::ua::spoof_ua())
        .header("Accept-Language", "ja-JP,ja;q=0.9,en-US;q=0.8,en;q=0.7")
}

struct NdlBookInfo {
    title: String,
    author: Option<String>,
    publisher: Option<String>,
    publish_date: Option<String>,
    description: Option<String>,
    title_transcription: Option<String>,
    creator_transcription: Option<String>,
    series_title: Option<String>,
    series_title_transcription: Option<String>,
    edition: Option<String>,
    price: Option<String>,
    extent: Option<String>,
    subject: Option<String>,
    ndl_url: Option<String>,
}

pub async fn lookup_isbn(client: &Client, isbn: &str) -> Result<Option<NewBook>, String> {
    let ndl = lookup_ndl(client, isbn).await?;
    let Some(ndl) = ndl else {
        return Ok(None);
    };

    let cover_url = lookup_amazon_cover(client, isbn).await.ok().flatten();

    Ok(Some(NewBook {
        isbn: isbn.to_string(),
        title: ndl.title,
        author: ndl.author,
        publisher: ndl.publisher,
        publish_date: ndl.publish_date,
        cover_url,
        description: ndl.description,
        title_transcription: ndl.title_transcription,
        creator_transcription: ndl.creator_transcription,
        series_title: ndl.series_title,
        series_title_transcription: ndl.series_title_transcription,
        edition: ndl.edition,
        price: ndl.price,
        extent: ndl.extent,
        subject: ndl.subject,
        ndl_url: ndl.ndl_url,
    }))
}

async fn lookup_amazon_cover(client: &Client, isbn: &str) -> Result<Option<String>, String> {
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

    Ok(cover_url)
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
    let url = format!(
        "https://ndlsearch.ndl.go.jp/api/opensearch?isbn={}&cnt=1",
        isbn
    );
    let body = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("NDL request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("NDL read failed: {}", e))?;
    parse_ndl_rss(&body)
}

fn parse_ndl_rss(xml: &str) -> Result<Option<NdlBookInfo>, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut in_item = false;
    let mut current_tag = String::new();
    let mut title: Option<String> = None;
    let mut author: Option<String> = None;
    let mut publisher: Option<String> = None;
    let mut pub_date: Option<String> = None;
    let mut description: Option<String> = None;
    let mut title_transcription: Option<String> = None;
    let mut creator_transcription: Option<String> = None;
    let mut series_title: Option<String> = None;
    let mut series_title_transcription: Option<String> = None;
    let mut edition: Option<String> = None;
    let mut price: Option<String> = None;
    let mut extent: Option<String> = None;
    let mut subject: Option<String> = None;
    let mut ndl_url: Option<String> = None;
    let mut buf = Vec::new();

    use quick_xml::events::Event;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "item" {
                    in_item = true;
                }
                current_tag = tag;
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "item" {
                    in_item = true;
                }
                current_tag = tag;
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "item" {
                    break;
                }
            }
            Ok(Event::Text(ref e)) if in_item => {
                let text = e.decode().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "title" if title.is_none() => title = Some(text),
                    "creator" if author.is_none() => author = Some(text),
                    "publisher" if publisher.is_none() => publisher = Some(text),
                    "issued" if pub_date.is_none() => pub_date = Some(text),
                    "description" if description.is_none() => description = Some(text),
                    "titleTranscription" if title_transcription.is_none() => title_transcription = Some(text),
                    "creatorTranscription" if creator_transcription.is_none() => creator_transcription = Some(text),
                    "seriesTitle" if series_title.is_none() => series_title = Some(text),
                    "seriesTitleTranscription" if series_title_transcription.is_none() => series_title_transcription = Some(text),
                    "edition" if edition.is_none() => edition = Some(text),
                    "price" if price.is_none() => price = Some(text),
                    "extent" if extent.is_none() => extent = Some(text),
                    "subject" if subject.is_none() => subject = Some(text),
                    "link" if ndl_url.is_none() => ndl_url = Some(text),
                    _ => {}
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
        author,
        publisher,
        publish_date: pub_date,
        description,
        title_transcription,
        creator_transcription,
        series_title,
        series_title_transcription,
        edition,
        price,
        extent,
        subject,
        ndl_url,
    }))
}
