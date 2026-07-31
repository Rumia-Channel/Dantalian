use crate::db::NewBook;
use reqwest::Client;
use tracing::debug;

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
    let xml = xml.to_string();
    let result = tokio::task::spawn_blocking(move || {
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
            (Some(s), Some(sz), Some(c), Some(cu)) => Some(format!("{}, {}, {}{}", s, sz, c, cu)),
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
        jan: None,
        title: info.title,
        publisher: info.publisher,
        publish_date: crate::external::normalize_publish_date(info.publish_date.as_deref()),
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
        media_type: None,
        catalog_number: None,
        artist: None,
        label: None,
        disc_count: None,
        tracks: None,
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
