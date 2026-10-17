use super::{NdlAuthor, NdlBook, local_name};
use dantalian::ndl_authority::entity_id;
use quick_xml::{Reader, events::Event};

pub(super) fn parse_sru(xml: &str) -> std::result::Result<Option<NdlBook>, String> {
    let mut best: Option<NdlBook> = None;
    let mut offset = 0;
    let mut saw_record = false;
    while let Some(start) = xml[offset..].find("<recordData") {
        let start = offset + start;
        let end = xml[start..]
            .find("</recordData>")
            .map(|end| start + end + "</recordData>".len())
            .ok_or_else(|| "NDL XML contains an unclosed record".to_string())?;
        if let Some(book) = parse_record(&xml[start..end])? {
            if best
                .as_ref()
                .is_none_or(|previous| score(&book) >= score(previous))
            {
                best = Some(book);
            }
        }
        saw_record = true;
        offset = end;
    }
    if saw_record {
        Ok(best)
    } else {
        parse_record(xml)
    }
}

pub(super) fn score(book: &NdlBook) -> usize {
    let author_ids = book
        .authors
        .iter()
        .filter(|author| author.ndl_id.is_some())
        .count();
    let ndl_record = book
        .ndl_url
        .as_deref()
        .is_some_and(|url| url.contains("R100000002"));
    let fields = [
        &book.publisher,
        &book.publish_date,
        &book.description,
        &book.title_transcription,
        &book.series_title,
        &book.series_title_transcription,
        &book.alternative,
        &book.price,
        &book.extent,
        &book.jpno,
        &book.ndl_url,
    ]
    .into_iter()
    .filter(|field| field.is_some())
    .count();
    author_ids * 100 + usize::from(ndl_record) * 50 + fields
}

fn prefixed_name(name: quick_xml::name::QName<'_>) -> Option<String> {
    name.prefix()
        .map(|prefix| String::from_utf8_lossy(prefix.as_ref()).to_string())
}

fn read_diagnostic(reader: &mut Reader<&[u8]>) -> std::result::Result<(), String> {
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
    dantalian::ndl_search::check_sru_diagnostic(
        uri.as_deref().unwrap_or(""),
        message.as_deref().unwrap_or(""),
    )
}

fn parse_record(xml: &str) -> std::result::Result<Option<NdlBook>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut in_record_data = false;
    let mut in_main_resource = false;
    let mut in_creator = false;
    let mut author_id: Option<String> = None;
    let mut author_name: Option<String> = None;
    let mut author_transcription: Option<String> = None;
    let mut identifier_is_jpno = false;
    let mut result = NdlBook::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name());
                let prefix = prefixed_name(event.name());
                if tag == "diagnostic" && !in_main_resource {
                    read_diagnostic(&mut reader)?;
                }
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
                    if tag == "identifier" && path.len() == 2 {
                        identifier_is_jpno = event.attributes().flatten().any(|attribute| {
                            attribute.key.local_name().as_ref() == b"datatype"
                                && attribute.value.ends_with(b"/JPNO")
                        });
                    }
                    if tag == "Agent" && in_creator {
                        author_id = event.attributes().flatten().find_map(|attribute| {
                            if attribute.key.local_name().as_ref() != b"about" {
                                return None;
                            }
                            entity_id(&String::from_utf8_lossy(&attribute.value))
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
                    && (path == &["BibResource", "title"]
                        || path == &["BibResource", "title", "Description", "value"])
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
                } else if path == &["BibResource", "date"]
                    || (result.publish_date.is_none() && path == &["BibResource", "issued"])
                {
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
                    && identifier_is_jpno
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
            Ok(Event::Empty(event))
                if !in_main_resource && local_name(event.name()) == "diagnostic" =>
            {
                return Err("NDL search returned an SRU diagnostic".to_string());
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("NDL XML parse failed: {error}")),
        }
        buf.clear();
    }
    result.publish_date = result.publish_date.take().and_then(|value| {
        dantalian::application::publish_date::normalize_publish_date(Some(&value))
    });

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
mod tests;
