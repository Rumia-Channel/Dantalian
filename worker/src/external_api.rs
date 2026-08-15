use futures_util::future::{Either, select};
use futures_util::pin_mut;
use quick_xml::{Reader, events::Event};
use std::time::Duration;
use url::Url;
use worker::{AbortController, Delay, Fetch, Result};

const EXTERNAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default)]
pub struct NdlAuthor {
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NdlBook {
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub alternative: Option<String>,
    pub alternative_transcription: Option<String>,
    pub volume: Option<String>,
    pub volume_transcription: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub jpno: Option<String>,
    pub ndl_url: Option<String>,
    pub cover_url: Option<String>,
    pub isdn_region: Option<String>,
    pub isdn_class: Option<String>,
    pub isdn_type: Option<String>,
    pub isdn_rating_gender: Option<String>,
    pub isdn_rating_age: Option<String>,
    pub isdn_genre_code: Option<String>,
    pub isdn_genre_name: Option<String>,
    pub isdn_genre_user: Option<String>,
    pub isdn_c_code: Option<String>,
    pub isdn_author: Option<String>,
    pub isdn_shape: Option<String>,
    pub isdn_contents: Option<String>,
    pub isdn_barcode2: Option<String>,
    pub isdn_sample_image_url: Option<String>,
    pub isdn_useroption: Option<String>,
    pub isdn_external_links: Option<String>,
    pub authors: Vec<NdlAuthor>,
}

pub(crate) async fn fetch_with_timeout(fetch: Fetch, label: &str) -> Result<worker::Response> {
    let controller = AbortController::default();
    let signal = controller.signal();
    let request = fetch.send_with_signal(&signal);
    let delay = Delay::from(EXTERNAL_REQUEST_TIMEOUT);
    pin_mut!(request, delay);

    match select(request, delay).await {
        Either::Left((result, _delay)) => result
            .map_err(|error| worker::Error::RustError(format!("{label} request failed: {error}"))),
        Either::Right((_timeout, request)) => {
            drop(request);
            controller.abort();
            Err(worker::Error::RustError(format!(
                "{label} request timed out after {} seconds",
                EXTERNAL_REQUEST_TIMEOUT.as_secs()
            )))
        }
    }
}

pub async fn lookup_isbn(isbn: &str) -> Result<Option<NdlBook>> {
    let mut url = Url::parse("https://ndlsearch.ndl.go.jp/api/sru")
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("operation", "searchRetrieve")
        .append_pair("version", "1.2")
        .append_pair("recordSchema", "dcndl")
        .append_pair("onlyBib", "true")
        .append_pair("maximumRecords", "10")
        .append_pair("startRecord", "1")
        .append_pair("recordPacking", "xml")
        .append_pair("query", &format!("isbn=\"{isbn}\""));
    let mut response = fetch_with_timeout(Fetch::Url(url), "NDL").await?;
    if !response.status_code().to_string().starts_with('2') {
        return Ok(None);
    }
    let xml = response
        .text()
        .await
        .map_err(|error| worker::Error::RustError(format!("NDL response read failed: {error}")))?;
    parse_sru(&xml).map_err(|error| worker::Error::RustError(error.to_string()))
}

pub async fn lookup_isdn(isdn: &str) -> Result<Option<NdlBook>> {
    let url = Url::parse(&format!("https://isdn.jp/xml/{isdn}"))
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    let mut response = fetch_with_timeout(Fetch::Url(url), "ISDN").await?;
    if !response.status_code().to_string().starts_with('2') {
        return Ok(None);
    }
    let xml = response
        .text()
        .await
        .map_err(|error| worker::Error::RustError(format!("ISDN response read failed: {error}")))?;
    parse_isdn(&xml).map_err(|error| worker::Error::RustError(error.to_string()))
}

fn local_name(name: quick_xml::name::QName<'_>) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).to_string()
}
fn parse_isdn(xml: &str) -> std::result::Result<Option<NdlBook>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut in_item = false;
    let mut result = NdlBook::default();
    let mut price_unit = None;
    let mut product_style = None;
    let mut product_size = None;
    let mut product_capacity = None;
    let mut product_capacity_unit = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name());
                if tag == "item" && !in_item {
                    in_item = true;
                    path.clear();
                    path.push(tag);
                } else if in_item {
                    path.push(tag);
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name());
                if in_item && tag == "item" && path.len() == 1 {
                    break;
                }
                if in_item && path.last().is_some_and(|value| value == &tag) {
                    path.pop();
                }
            }
            Ok(Event::Text(event)) if in_item => {
                let text = event
                    .decode()
                    .map_err(|error| format!("ISDN XML text decode failed: {error}"))?
                    .trim()
                    .to_string();
                if text.is_empty() || path.len() < 2 {
                    buf.clear();
                    continue;
                }
                match path[1].as_str() {
                    "product-name" if result.title.is_empty() => result.title = text,
                    "product-yomi" if result.title_transcription.is_none() => {
                        result.title_transcription = Some(text)
                    }
                    "publisher-name" if result.publisher.is_none() => result.publisher = Some(text),
                    "issue-date" if result.publish_date.is_none() => {
                        result.publish_date = Some(text)
                    }
                    "product-comment" if result.description.is_none() => {
                        result.description = Some(text)
                    }
                    "price" if result.price.is_none() => result.price = Some(text),
                    "price-unit" if price_unit.is_none() => price_unit = Some(text),
                    "product-style" if product_style.is_none() => product_style = Some(text),
                    "product-size" if product_size.is_none() => product_size = Some(text),
                    "product-capacity" if product_capacity.is_none() => {
                        product_capacity = Some(text)
                    }
                    "product-capacity-unit" if product_capacity_unit.is_none() => {
                        product_capacity_unit = Some(text)
                    }
                    "sample-image-uri" if result.cover_url.is_none() => {
                        result.cover_url = Some(text.clone());
                        result.isdn_sample_image_url = Some(text);
                    }
                    "region" if result.isdn_region.is_none() => result.isdn_region = Some(text),
                    "class" if result.isdn_class.is_none() => result.isdn_class = Some(text),
                    "type" if result.isdn_type.is_none() => result.isdn_type = Some(text),
                    "rating_gender" if result.isdn_rating_gender.is_none() => {
                        result.isdn_rating_gender = Some(text)
                    }
                    "rating_age" if result.isdn_rating_age.is_none() => {
                        result.isdn_rating_age = Some(text)
                    }
                    "genre-code" if result.isdn_genre_code.is_none() => {
                        result.isdn_genre_code = Some(text)
                    }
                    "genre-name" if result.isdn_genre_name.is_none() => {
                        result.isdn_genre_name = Some(text)
                    }
                    "genre-user" if result.isdn_genre_user.is_none() => {
                        result.isdn_genre_user = Some(text)
                    }
                    "c-code" if result.isdn_c_code.is_none() => result.isdn_c_code = Some(text),
                    "author" if result.isdn_author.is_none() => {
                        result.authors.push(NdlAuthor {
                            ndl_id: None,
                            name: text.clone(),
                            transcription: None,
                        });
                        result.isdn_author = Some(text);
                    }
                    "shape" if result.isdn_shape.is_none() => result.isdn_shape = Some(text),
                    "contents" if result.isdn_contents.is_none() => {
                        result.isdn_contents = Some(text)
                    }
                    "barcode2" if result.isdn_barcode2.is_none() => {
                        result.isdn_barcode2 = Some(text)
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("ISDN XML parse failed: {error}")),
        }
        buf.clear();
    }

    if result.title.is_empty() {
        return Ok(None);
    }
    if result.price.is_some() {
        if let (Some(price), Some(unit)) = (result.price.take(), price_unit) {
            result.price = Some(format!("{price} {unit}"));
        }
    }
    let extent = [
        product_style,
        product_size,
        product_capacity
            .zip(product_capacity_unit)
            .map(|(value, unit)| format!("{value}{unit}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !extent.is_empty() {
        result.extent = Some(extent.join(", "));
        if result.description.is_none() {
            result.description = result.extent.clone();
        }
    }
    Ok(Some(result))
}

fn prefixed_name(name: quick_xml::name::QName<'_>) -> Option<String> {
    name.prefix()
        .map(|prefix| String::from_utf8_lossy(prefix.as_ref()).to_string())
}

fn parse_sru(xml: &str) -> std::result::Result<Option<NdlBook>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut in_record_data = false;
    let mut in_main_resource = false;
    let mut in_creator = false;
    let mut author_id: Option<String> = None;
    let mut author_name: Option<String> = None;
    let mut author_transcription: Option<String> = None;
    let mut result = NdlBook::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name());
                let prefix = prefixed_name(event.name());
                if tag == "recordData" {
                    in_record_data = true;
                    path.clear();
                } else if in_record_data && !in_main_resource && tag == "BibResource" {
                    let is_material = event.attributes().flatten().any(|attribute| {
                        attribute.key.local_name().as_ref() == b"about"
                            && attribute
                                .value
                                .windows(9)
                                .any(|value| value == b"#material")
                    });
                    if is_material {
                        in_main_resource = true;
                        result.ndl_url = event.attributes().flatten().find_map(|attribute| {
                            (attribute.key.local_name().as_ref() == b"about")
                                .then(|| String::from_utf8_lossy(&attribute.value).to_string())
                        });
                        path.clear();
                        path.push(tag);
                    }
                } else if in_main_resource {
                    if tag == "creator" && prefix.as_deref() == Some("dcterms") {
                        in_creator = true;
                        author_id = None;
                        author_name = None;
                        author_transcription = None;
                    }
                    path.push(tag.clone());
                    if tag == "Agent" && in_creator {
                        author_id = event.attributes().flatten().find_map(|attribute| {
                            if attribute.key.local_name().as_ref() != b"about" {
                                return None;
                            }
                            String::from_utf8_lossy(&attribute.value)
                                .rsplit('/')
                                .next()
                                .map(str::to_string)
                        });
                    }
                }
            }
            Ok(Event::End(event)) => {
                let tag = local_name(event.name());
                let prefix = prefixed_name(event.name());
                if in_record_data && tag == "recordData" {
                    break;
                }
                if in_main_resource && tag == "BibResource" && path.len() == 1 {
                    in_main_resource = false;
                }
                if in_main_resource && path.last().is_some_and(|value| value == &tag) {
                    path.pop();
                }
                if tag == "creator" && prefix.as_deref() == Some("dcterms") && in_creator {
                    in_creator = false;
                    if let Some(name) = author_name.take().filter(|name| !name.is_empty()) {
                        result.authors.push(NdlAuthor {
                            ndl_id: author_id.take(),
                            name,
                            transcription: author_transcription.take(),
                        });
                    }
                }
            }
            Ok(Event::Text(event)) if in_main_resource => {
                let text = event
                    .decode()
                    .map_err(|error| format!("NDL XML text decode failed: {error}"))?
                    .trim()
                    .to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }
                let path = &path;
                if result.title.is_empty()
                    && path == &["BibResource", "title", "Description", "value"]
                {
                    result.title = text;
                } else if in_creator
                    && author_name.is_none()
                    && path == &["BibResource", "creator", "Agent", "name"]
                {
                    author_name = Some(clean_author(&text));
                } else if in_creator
                    && author_transcription.is_none()
                    && path == &["BibResource", "creator", "Agent", "transcription"]
                {
                    author_transcription = Some(clean_author(&text));
                } else if result.publisher.is_none()
                    && path == &["BibResource", "publisher", "Agent", "name"]
                {
                    result.publisher = Some(text);
                } else if result.publish_date.is_none() && path == &["BibResource", "date"] {
                    result.publish_date = Some(text);
                } else if result.title_transcription.is_none()
                    && path == &["BibResource", "title", "Description", "transcription"]
                {
                    result.title_transcription = Some(text);
                } else if result.series_title_transcription.is_none()
                    && path == &["BibResource", "seriesTitle", "Description", "transcription"]
                {
                    result.series_title_transcription = Some(text);
                } else if result.series_title.is_none()
                    && path == &["BibResource", "seriesTitle", "Description", "value"]
                {
                    result.series_title = Some(text);
                } else if result.price.is_none() && path == &["BibResource", "price"] {
                    result.price = Some(text);
                } else if result.extent.is_none() && path == &["BibResource", "extent"] {
                    result.extent = Some(text);
                } else if result.alternative.is_none()
                    && path == &["BibResource", "alternative", "Description", "value"]
                {
                    result.alternative = Some(text);
                } else if result.alternative_transcription.is_none()
                    && path == &["BibResource", "alternative", "Description", "transcription"]
                {
                    result.alternative_transcription = Some(text);
                } else if result.volume.is_none()
                    && path == &["BibResource", "volume", "Description", "value"]
                {
                    result.volume = Some(text);
                } else if result.volume_transcription.is_none()
                    && path == &["BibResource", "volume", "Description", "transcription"]
                {
                    result.volume_transcription = Some(text);
                } else if result.jpno.is_none()
                    && path == &["BibResource", "identifier"]
                    && text.chars().all(|value| value.is_ascii_digit())
                {
                    result.jpno = Some(text);
                } else if result.description.is_none()
                    && path == &["BibResource", "description"]
                    && !text.starts_with("表現種別")
                    && !text.starts_with("機器種別")
                    && !text.starts_with("キャリア種別")
                {
                    result.description = Some(text);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("NDL XML parse failed: {error}")),
        }
        buf.clear();
    }

    if result.title.is_empty() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

fn clean_author(value: &str) -> String {
    value.replace([',', '\u{FF0C}'], "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_isdn, parse_sru};

    #[test]
    fn parses_material_resource_and_author() {
        let xml = r#"
            <response><records><record><recordData>
              <dcndl:BibResource rdf:about="https://ndl.example/#material">
                <dcndl:title><rdf:Description><rdf:value>Title</rdf:value><dcndl:transcription>Yomi</dcndl:transcription></rdf:Description></dcndl:title>
                <dcterms:creator><foaf:Agent rdf:about="https://id.ndl.go.jp/auth/1"><foaf:name>Author</foaf:name></foaf:Agent></dcterms:creator>
                <dcterms:publisher><foaf:Agent><foaf:name>Publisher</foaf:name></foaf:Agent></dcterms:publisher>
                <dcterms:date>2024</dcterms:date>
              </dcndl:BibResource>
            </recordData></record></records></response>
        "#;
        let parsed = parse_sru(xml).unwrap().unwrap();
        assert_eq!(parsed.title, "Title");
        assert_eq!(parsed.publisher.as_deref(), Some("Publisher"));
        assert_eq!(parsed.authors[0].name, "Author");
        assert_eq!(parsed.authors[0].ndl_id.as_deref(), Some("1"));
    }

    #[test]
    fn parses_isdn_fields() {
        let xml = r#"
            <response><item>
              <product-name>Media title</product-name>
              <publisher-name>Publisher</publisher-name>
              <issue-date>2024-01-02</issue-date>
              <price>1200</price><price-unit>円</price-unit>
              <product-style>Paperback</product-style>
              <product-size>A5</product-size>
              <author>Author</author>
              <sample-image-uri>https://example.test/cover.jpg</sample-image-uri>
            </item></response>
        "#;
        let parsed = parse_isdn(xml).unwrap().unwrap();
        assert_eq!(parsed.title, "Media title");
        assert_eq!(parsed.price.as_deref(), Some("1200 円"));
        assert_eq!(parsed.extent.as_deref(), Some("Paperback, A5"));
        assert_eq!(parsed.isdn_author.as_deref(), Some("Author"));
        assert_eq!(
            parsed.cover_url.as_deref(),
            Some("https://example.test/cover.jpg")
        );
    }
}
