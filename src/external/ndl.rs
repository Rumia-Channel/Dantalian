use crate::db::NewAuthor;
use reqwest::Client;
use tracing::{debug, warn};

pub(crate) struct NdlBookInfo {
    pub(crate) title: String,
    pub(crate) publisher: Option<String>,
    pub(crate) publish_date: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) title_transcription: Option<String>,
    pub(crate) series_title: Option<String>,
    pub(crate) series_title_transcription: Option<String>,
    pub(crate) alternative: Option<String>,
    pub(crate) alternative_transcription: Option<String>,
    pub(crate) volume: Option<String>,
    pub(crate) volume_transcription: Option<String>,
    pub(crate) price: Option<String>,
    pub(crate) extent: Option<String>,
    pub(crate) jpno: Option<String>,
    pub(crate) ndl_url: Option<String>,
    pub(crate) authors: Vec<NewAuthor>,
}

fn author_name_variants(name: &str) -> Vec<String> {
    let clean = name.trim();
    let mut variants = vec![clean.to_string()];
    let no_space = clean.replace([' ', '\u{3000}'], "");
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

pub(crate) async fn lookup_ndl(
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
                    cur_author_name = Some(text.replace(',', "").replace('\u{FF0C}', ""));
                } else if in_dcterms_creator
                    && cur_author_transcription.is_none()
                    && p.len() == 4
                    && p[1] == "creator"
                    && p[2] == "Agent"
                    && p[3] == "transcription"
                {
                    cur_author_transcription = Some(text.replace(',', "").replace('\u{FF0C}', ""));
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

pub(crate) async fn enrich_author_ndl_ids(client: &Client, authors: &mut [NewAuthor]) {
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
