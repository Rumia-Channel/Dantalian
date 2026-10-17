use crate::db::NewAuthor;
use crate::external::book_metadata::BookMetadata;
use crate::ndl_authority::entity_id;

pub(super) fn parse_candidates(xml: &str) -> Result<Vec<BookMetadata>, String> {
    let mut candidates = Vec::new();
    let mut offset = 0;

    while let Some(start_rel) = xml[offset..].find("<recordData") {
        let start = offset + start_rel;
        let open_end_rel = xml[start..]
            .find('>')
            .ok_or_else(|| "NDL XML contains an unclosed record".to_string())?;
        let content_start = start + open_end_rel + 1;
        let end_rel = xml[content_start..]
            .find("</recordData>")
            .ok_or_else(|| "NDL XML contains an unclosed record".to_string())?;
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

fn read_diagnostic(reader: &mut quick_xml::Reader<&[u8]>) -> Result<(), String> {
    use quick_xml::events::Event;
    let mut uri = None;
    let mut message = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) if event.name().local_name().as_ref() == b"uri" => {
                uri = Some(
                    reader
                        .read_text(event.name())
                        .map_err(|error| format!("NDL diagnostic URI read failed: {error}"))?,
                );
            }
            Ok(Event::Start(event)) if event.name().local_name().as_ref() == b"message" => {
                message = Some(
                    reader
                        .read_text(event.name())
                        .map_err(|error| format!("NDL diagnostic message read failed: {error}"))?,
                );
            }
            Ok(Event::End(event)) if event.name().local_name().as_ref() == b"diagnostic" => break,
            Ok(Event::Eof) => return Err("NDL XML contains an unclosed diagnostic".to_string()),
            Err(error) => return Err(format!("NDL diagnostic XML parse failed: {error}")),
            _ => {}
        }
    }
    let uri = uri
        .as_ref()
        .map(|text| text.decode())
        .transpose()
        .map_err(|error| format!("NDL diagnostic URI decode failed: {error}"))?;
    let message = message
        .as_ref()
        .map(|text| text.decode())
        .transpose()
        .map_err(|error| format!("NDL diagnostic message decode failed: {error}"))?;
    crate::ndl_search::check_sru_diagnostic(
        uri.as_deref().unwrap_or(""),
        message.as_deref().unwrap_or(""),
    )
}

fn parse_ndl_sru_record(xml: &str) -> Result<Option<BookMetadata>, String> {
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
    let mut identifier_is_jpno = false;

    let mut buf = Vec::new();

    use quick_xml::events::Event;

    loop {
        match &reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().local_name().as_ref()).to_string();
                let prefix = e
                    .name()
                    .prefix()
                    .map(|p| String::from_utf8_lossy(p.as_ref()).to_string());
                if tag == "diagnostic" && !in_main_resource {
                    read_diagnostic(&mut reader)?;
                }

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
                    if tag == "identifier" && path.len() == 2 {
                        identifier_is_jpno = e.attributes().flatten().any(|attribute| {
                            attribute.key.local_name().as_ref() == b"datatype"
                                && attribute.value.ends_with(b"/JPNO")
                        });
                    }

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
                                cur_author_ndl_id = entity_id(&full).map(str::to_string);
                                break;
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(e))
                if !in_main_resource && e.name().local_name().as_ref() == b"diagnostic" =>
            {
                return Err("NDL search returned an SRU diagnostic".to_string());
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::End(e)) => {
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
            Ok(Event::Text(e)) if in_main_resource => {
                let text = e.decode().unwrap_or_default().to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }

                let p = &path;

                if title.is_none()
                    && (p.as_slice() == ["BibResource", "title"]
                        || p.as_slice() == ["BibResource", "title", "Description", "value"])
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
                } else if p.as_slice() == ["BibResource", "date"]
                    || (pub_date.is_none() && p.as_slice() == ["BibResource", "issued"])
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
                    && identifier_is_jpno
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

    Ok(Some(BookMetadata {
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
