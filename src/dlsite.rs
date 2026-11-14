use serde::{Deserialize, Serialize};

pub fn normalize_product_id(value: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let candidate = if let Some((scheme, rest)) = value.split_once("://") {
        let (host, path) = rest.split_once('/').ok_or("Invalid DLsite work URL")?;
        if !matches!(scheme, "https" | "http")
            || !["www.dlsite.com", "dlsite.com"]
                .iter()
                .any(|allowed| host.eq_ignore_ascii_case(allowed))
        {
            return Err("Use a DLsite ID or a dlsite.com work URL");
        }
        path.split_once("/product_id/")
            .map(|(_, id)| id)
            .ok_or("Invalid DLsite work URL")?
            .split(['?', '#', '/'])
            .next()
            .unwrap_or("")
    } else {
        value
    };
    let upper = candidate.to_ascii_uppercase();
    let id = upper.strip_suffix(".HTML").unwrap_or(&upper);
    let bytes = id.as_bytes();
    if !(8..=10).contains(&bytes.len())
        || !bytes[..2].iter().all(u8::is_ascii_alphabetic)
        || !bytes[2..].iter().all(u8::is_ascii_digit)
    {
        return Err("DLsite ID must be two ASCII letters followed by 6 to 8 digits");
    }
    Ok(Some(id.to_owned()))
}

pub fn work_url(id: &str) -> String {
    format!("https://www.dlsite.com/home/work/=/product_id/{id}.html")
}

pub fn info_url(id: &str) -> String {
    format!("https://www.dlsite.com/home/product/info/ajax?product_id={id}")
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DlsiteMediaType {
    Book,
    Cd,
    Audiobook,
}

impl DlsiteMediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Book => "book",
            Self::Cd => "cd",
            Self::Audiobook => "audiobook",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DlsiteRegisterRequest {
    pub dlsite_id: String,
    pub media_type: DlsiteMediaType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DlsiteTrack {
    pub title: String,
    pub duration: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DlsiteWork {
    pub dlsite_id: String,
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub description: Option<String>,
    pub series_title: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub authors: Vec<String>,
    pub cover_url: Option<String>,
    pub voice_actors: Vec<String>,
    pub tracks: Vec<DlsiteTrack>,
}

#[cfg(any(feature = "native", feature = "dlsite"))]
pub fn parse_work(
    id: &str,
    info: &serde_json::Value,
    html: &str,
) -> Result<Option<DlsiteWork>, String> {
    use scraper::{Html, Selector};
    use std::sync::LazyLock;
    struct Selectors {
        title: Selector,
        publisher: Selector,
        rows: Selector,
        heading: Selector,
        value: Selector,
        links: Selector,
        description: Selector,
        image: Selector,
        tracks: Selector,
        track_title: Selector,
        track_time: Selector,
    }
    static S: LazyLock<Selectors> = LazyLock::new(|| {
        Selectors {
        title: Selector::parse("#work_name").unwrap(),
        publisher: Selector::parse(
            "#work_maker .maker_name, .c-productInfo__gridContext .c-productInfo__makerName",
        )
        .unwrap(),
        rows: Selector::parse("#work_outline tr, .c-productInfo__gridContext .c-productInfo__item")
            .unwrap(),
        heading: Selector::parse("th, dt").unwrap(),
        value: Selector::parse("td, dd").unwrap(),
        links: Selector::parse("a").unwrap(),
        description: Selector::parse("[itemprop=description], .c-productContent").unwrap(),
        image: Selector::parse("meta[property='og:image']").unwrap(),
        tracks: Selector::parse(
            ".work_parts.type_tracklist .work_tracklist_item, .c-productContent .c-productContent__trackListItem",
        )
        .unwrap(),
        track_title: Selector::parse(".title, .c-productContent__trackItemTitle").unwrap(),
        track_time: Selector::parse(".time, .c-productContent__trackItemTime").unwrap(),
    }
    });
    fn append_text(result: &mut String, node: scraper::ElementRef<'_>) {
        for piece in node.text().flat_map(str::split_whitespace) {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(piece);
        }
    }
    fn text(node: scraper::ElementRef<'_>) -> String {
        let mut result = String::new();
        append_text(&mut result, node);
        result
    }
    let Some(info) = info.get(id).filter(|value| value.is_object()) else {
        return Ok(None);
    };
    let document = Html::parse_document(html);
    let title = document
        .select(&S.title)
        .next()
        .map(text)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "DLsite work page did not contain a title (access may be restricted)".to_owned()
        })?;
    let mut description = String::new();
    for node in document.select(&S.description) {
        append_text(&mut description, node);
    }
    let mut work = DlsiteWork {
        dlsite_id: id.to_owned(),
        title,
        publisher: document
            .select(&S.publisher)
            .next()
            .map(text)
            .filter(|value| !value.is_empty()),
        publish_date: info
            .get("regist_date")
            .and_then(|value| value.as_str())
            .and_then(|value| value.get(..10))
            .map(str::to_owned),
        description: (!description.is_empty()).then_some(description),
        series_title: info
            .get("title_name")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        price: info
            .get("official_price")
            .or_else(|| info.get("price"))
            .and_then(|value| value.as_u64())
            .map(|price| price.to_string()),
        extent: None,
        authors: Vec::new(),
        cover_url: None,
        voice_actors: Vec::new(),
        tracks: Vec::new(),
    };
    for row in document.select(&S.rows) {
        let label = row.select(&S.heading).next().map(text).unwrap_or_default();
        let Some(value) = row.select(&S.value).next() else {
            continue;
        };
        match label.as_str() {
            "作者" | "著者" | "原作" | "イラスト" | "シナリオ" | "声優" | "音楽" => {
                for name in value
                    .select(&S.links)
                    .map(text)
                    .filter(|name| !name.is_empty())
                {
                    if label == "声優" && !work.voice_actors.contains(&name) {
                        work.voice_actors.push(name.clone());
                    }
                    if !work.authors.contains(&name) {
                        work.authors.push(name);
                    }
                }
            }
            "ページ数" => {
                work.extent = Some(text(value));
            }
            "シリーズ名" if work.series_title.is_none() => {
                work.series_title = Some(text(value));
            }
            _ => {}
        }
    }
    if work.authors.is_empty() {
        work.authors.extend(work.publisher.iter().cloned());
    }
    for track in document.select(&S.tracks) {
        let Some(title) = track
            .select(&S.track_title)
            .next()
            .map(text)
            .filter(|title| !title.is_empty())
        else {
            continue;
        };
        let duration = track
            .select(&S.track_time)
            .next()
            .map(text)
            .filter(|value| !value.is_empty());
        work.tracks.push(DlsiteTrack { title, duration });
    }
    let image = info
        .get("work_image")
        .and_then(|value| value.as_str())
        .or_else(|| {
            document
                .select(&S.image)
                .next()
                .and_then(|node| node.value().attr("content"))
        });
    if let Some(image) = image {
        let image = if image.starts_with("//") {
            format!("https:{image}")
        } else {
            image.to_owned()
        };
        if ["https://img.dlsite.jp/", "https://img.dlsite.com/"]
            .iter()
            .any(|prefix| image.starts_with(prefix))
        {
            work.cover_url = Some(image);
        }
    }
    Ok(Some(work))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_identity_accepts_urls_and_rejects_untrusted_or_malformed_values() {
        assert_eq!(
            normalize_product_id(Some(" rj01676238 "))
                .unwrap()
                .as_deref(),
            Some("RJ01676238")
        );
        assert_eq!(
            normalize_product_id(Some(
                "https://www.dlsite.com/books/work/=/product_id/BJ123456.html?locale=ja_JP"
            ))
            .unwrap()
            .as_deref(),
            Some("BJ123456")
        );
        for invalid in [
            "RJ1",
            "RJ123456789",
            "1J123456",
            "RJ１２３４５６",
            "https://evil.example/work/=/product_id/RJ01676238.html",
            "RJ01676238<script>",
        ] {
            assert!(normalize_product_id(Some(invalid)).is_err(), "{invalid}");
        }
        assert_eq!(
            normalize_product_id(Some("XY123456")).unwrap().as_deref(),
            Some("XY123456")
        );
        assert_eq!(normalize_product_id(Some(" ")).unwrap(), None);
    }
    #[cfg(any(feature = "native", feature = "dlsite"))]
    #[test]
    fn metadata_uses_work_body_not_advertisements_and_keeps_multiple_creators() {
        let html = r#"<h1 id="work_name">Book &amp; Story</h1><table id="work_maker"><tr><td><span class="maker_name">Circle</span></td></tr></table><table id="work_outline"><tr><th>作者</th><td><a>Alice</a> / <a>Bob</a></td></tr></table><div itemprop="description"><p>First paragraph.</p><p>Second paragraph.</p></div>"#;
        let info = serde_json::json!({"RJ01676238": {"regist_date":"2026-07-25 00:00:00","official_price":770,"price":577,"work_image":"//img.dlsite.jp/work/cover.jpg"}});
        let work = parse_work("RJ01676238", &info, html).unwrap().unwrap();
        assert_eq!(work.title, "Book & Story");
        assert_eq!(work.authors, ["Alice", "Bob"]);
        assert_eq!(work.publisher.as_deref(), Some("Circle"));
        assert_eq!(work.price.as_deref(), Some("770"));
        assert_eq!(work.publish_date.as_deref(), Some("2026-07-25"));
        assert_eq!(
            work.description.as_deref(),
            Some("First paragraph. Second paragraph.")
        );
        assert_eq!(
            work.cover_url.as_deref(),
            Some("https://img.dlsite.jp/work/cover.jpg")
        );
        assert!(parse_work("RJ99999999", &info, html).unwrap().is_none());
        assert!(parse_work("RJ01676238", &info, "<h1>Access denied</h1>").is_err());
    }

    #[cfg(any(feature = "native", feature = "dlsite"))]
    #[test]
    fn authorless_work_uses_circle_without_follow_controls_as_author() {
        let info = serde_json::json!({"RJ01437353": {}});
        for maker in [
            r#"<table id="work_maker"><tr><td><span class="maker_name">グロウクローバー</span></td></tr></table>"#,
            r#"<dl class="c-productInfo__gridContext c-productInfoPc__list"><div class="c-productInfo__item"><dt>サークル名</dt><dd><a class="c-productInfo__makerName">グロウクローバー</a><a>フォローする</a><span>312</span></dd></div></dl>"#,
        ] {
            let html = format!(r#"<h1 id="work_name">彼女VS元カノ</h1>{maker}"#);
            let work = parse_work("RJ01437353", &info, &html).unwrap().unwrap();
            assert_eq!(work.publisher.as_deref(), Some("グロウクローバー"));
            assert_eq!(work.authors, ["グロウクローバー"]);
            assert!(work.voice_actors.is_empty());
        }
        let work = parse_work(
            "RJ01437353",
            &info,
            r#"<h1 id="work_name">Work without credits or circle</h1>"#,
        )
        .unwrap()
        .unwrap();
        assert!(work.authors.is_empty());
        assert!(work.publisher.is_none());
    }

    #[cfg(any(feature = "native", feature = "dlsite"))]
    #[test]
    fn current_layout_preserves_credits_description_and_tracks() {
        let info = serde_json::json!({"RJ123456": {}});
        let html = r#"<h1 id="work_name">Audio</h1>
        <dl class="c-productInfo__gridContext c-productInfoPc__list">
          <div class="c-productInfo__item"><dt>サークル名</dt><dd><a class="c-productInfo__makerName">Circle</a><a>Follow</a></dd></div>
          <div class="c-productInfo__item"><dt>作者</dt><dd><a>Writer</a></dd></div>
          <div class="c-productInfo__item"><dt>声優</dt><dd><a>Voice</a><a>Writer</a></dd></div>
          <div class="c-productInfo__item"><dt>ページ数</dt><dd>27P</dd></div>
          <div class="c-productInfo__item"><dt>シリーズ名</dt><dd>Series</dd></div>
        </dl>
        <aside><dl><div><dt>作者</dt><dd><a>Advertiser</a></dd></div></dl></aside>
        <section class="c-productContent"><h3>Synopsis</h3><div class="c-productContent__text">First paragraph.</div></section>
        <section class="c-productContent"><div class="c-productContent__text">Second paragraph.</div>
          <ul class="c-productContent__trackList">
            <li class="c-productContent__trackListItem"><div class="c-productContent__trackItemTitle">Opening</div><div class="c-productContent__trackItemTime">4:35</div></li>
            <li class="c-productContent__trackListItem"><div class="c-productContent__trackItemTitle">Closing</div><div class="c-productContent__trackItemTime"></div></li>
          </ul>
        </section>
        <section class="sample"><audio src="https://media.example/sample.mp3"></audio>
          <div class="c-productContent__trackListItem"><div class="c-productContent__trackItemTitle">Preview</div></div>
        </section>"#;
        let work = parse_work("RJ123456", &info, html).unwrap().unwrap();
        assert_eq!(work.publisher.as_deref(), Some("Circle"));
        assert_eq!(work.authors, ["Writer", "Voice"]);
        assert_eq!(work.voice_actors, ["Voice", "Writer"]);
        assert_eq!(work.extent.as_deref(), Some("27P"));
        assert_eq!(work.series_title.as_deref(), Some("Series"));
        assert_eq!(
            work.description.as_deref(),
            Some("Synopsis First paragraph. Second paragraph. Opening 4:35 Closing")
        );
        let tracks = work
            .tracks
            .iter()
            .map(|track| (track.title.as_str(), track.duration.as_deref()))
            .collect::<Vec<_>>();
        assert_eq!(tracks, [("Opening", Some("4:35")), ("Closing", None)]);
    }

    #[cfg(any(feature = "native", feature = "dlsite"))]
    #[test]
    fn audio_tracks_preserve_order_and_missing_times_without_inventing_tracks() {
        let info = serde_json::json!({"RJ123456": {}});
        let html = r#"<h1 id="work_name">Audio</h1>
        <table id="work_outline">
          <tr><th>作者</th><td><a>Alice</a></td></tr>
          <tr><th>声優</th><td><a>Alice</a><a>Bob</a></td></tr>
          <tr><th>シナリオ</th><td><a>Writer</a></td></tr>
        </table>
        <div class="work_parts type_tracklist"><ul>
          <li class="work_tracklist_item"><div class="title">Opening &amp; greeting</div><div class="time"></div></li>
          <li class="work_tracklist_item"><div class="title">Long track</div><div class="time">01:00:42</div></li>
          <li class="work_tracklist_item"><div class="title">Closing</div><div class="time">4:35</div></li>
        </ul></div>
        <div class="advertisement"><div class="work_tracklist_item"><div class="title">Unrelated</div></div></div>"#;
        let work = parse_work("RJ123456", &info, html).unwrap().unwrap();
        assert_eq!(work.voice_actors, ["Alice", "Bob"]);
        assert_eq!(work.authors, ["Alice", "Bob", "Writer"]);
        let tracks = work
            .tracks
            .iter()
            .map(|track| (track.title.as_str(), track.duration.as_deref()))
            .collect::<Vec<_>>();
        assert_eq!(
            tracks,
            [
                ("Opening & greeting", None),
                ("Long track", Some("01:00:42")),
                ("Closing", Some("4:35")),
            ]
        );
        let work = parse_work("RJ123456", &info, r#"<h1 id="work_name">No tracklist</h1><div itemprop="description">Listen to the full work.</div>"#).unwrap().unwrap();
        assert!(work.tracks.is_empty());
    }

    #[cfg(any(feature = "native", feature = "dlsite"))]
    #[test]
    fn preview_players_do_not_create_track_rows() {
        let info = serde_json::json!({"RJ123456": {}});
        let html = r#"<h1 id="work_name">Work without a tracklist</h1>
        <section class="sample">
          <h3>無料公開・試聴</h3>
          <audio controls src="https://media.example/sample.mp3"></audio>
          <a href="https://media.example/sample.zip">Sample download</a>
          <div class="work_tracklist_item"><div class="title">Free preview</div><div class="time">3:00</div></div>
        </section>"#;
        let work = parse_work("RJ123456", &info, html).unwrap().unwrap();
        assert!(work.tracks.is_empty());
    }
}
