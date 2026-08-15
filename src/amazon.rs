#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AmazonInfo {
    pub title: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub publish_date: Option<String>,
    pub isbn13: Option<String>,
}

pub fn isbn10_to_isbn13(isbn10: &str) -> Option<String> {
    let clean = isbn10.trim().replace(['-', ' '], "").to_uppercase();
    if clean.len() != 10 {
        return None;
    }
    let body = &clean[..9];
    if !body.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let mut digits = format!("978{body}");
    let sum: u32 = digits
        .chars()
        .enumerate()
        .map(|(index, character)| {
            character.to_digit(10).unwrap_or(0) * if index % 2 == 0 { 1 } else { 3 }
        })
        .sum();
    let check = (10 - (sum % 10)) % 10;
    digits.push(char::from_digit(check, 10)?);
    Some(digits)
}

pub fn isbn13_to_isbn10(isbn13: &str) -> Option<String> {
    let digits: String = isbn13.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 13 || !digits.starts_with("978") {
        return None;
    }
    let body = &digits[3..12];
    let sum: u32 = body
        .chars()
        .enumerate()
        .map(|(index, digit)| (10 - index as u32) * digit.to_digit(10).unwrap_or(0))
        .sum();
    let check = match 11 - (sum % 11) {
        10 => 'X',
        11 => '0',
        value => char::from_digit(value, 10)?,
    };
    Some(format!("{body}{check}"))
}

pub fn isbn_lookup_variants(isbn: &str) -> Vec<String> {
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

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

pub fn amazon_search_urls(keys: &[&str]) -> Vec<String> {
    let mut urls = Vec::new();
    let mut seen = Vec::new();
    for key in keys {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let variants = isbn_lookup_variants(key);
        for variant in variants {
            if !seen.contains(&variant) {
                seen.push(variant);
            }
        }
    }

    for key in seen {
        let encoded = percent_encode(&key);
        for (path, query) in [("/s", "k"), ("/gp/search/", "keywords"), ("/gp/aw/s", "k")] {
            urls.push(format!(
                "https://www.amazon.co.jp{path}?{query}={encoded}&i=stripbooks"
            ));
        }
    }
    urls
}

pub fn amazon_search_url(key: &str) -> Option<String> {
    amazon_search_urls(&[key]).into_iter().next()
}

fn href_marker(html: &str, cursor: usize) -> Option<(usize, char)> {
    let double = html[cursor..]
        .find("href=\"")
        .map(|offset| (cursor + offset, '"'));
    let single = html[cursor..]
        .find("href='")
        .map(|offset| (cursor + offset, '\''));
    match (double, single) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn is_nonphysical_product_anchor(html: &str, href_start: usize) -> bool {
    let anchor = &html[href_start..];
    let anchor = anchor
        .split_once("</a>")
        .map(|(anchor, _)| anchor)
        .unwrap_or(anchor)
        .to_ascii_lowercase();
    [
        "kindle",
        "audible",
        "ebook",
        "e-book",
        "digital",
        "電子書籍",
        "オーディオブック",
    ]
    .into_iter()
    .any(|marker| anchor.contains(marker))
}

fn product_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let mut cursor = 0;
    while let Some((start, quote)) = href_marker(html, cursor) {
        let value_start = start + 6;
        let Some(end) = html[value_start..]
            .find(quote)
            .map(|offset| value_start + offset)
        else {
            break;
        };
        let href = &html[value_start..end];
        if is_physical_product_href(href) && !is_nonphysical_product_anchor(html, start) {
            let href = href.replace("&amp;", "&");
            if !hrefs.iter().any(|value| value == &href) {
                hrefs.push(href);
            }
        }
        cursor = end + 1;
    }
    hrefs
}

pub fn is_physical_product_href(href: &str) -> bool {
    let path = href
        .split(['?', '#'])
        .next()
        .unwrap_or(href)
        .to_ascii_lowercase();
    if path.starts_with("http") && !path.starts_with("https://www.amazon.co.jp/") {
        return false;
    }
    let Some((product_path, _)) = path
        .split_once("/dp/")
        .or_else(|| path.split_once("/gp/aw/d/"))
    else {
        return false;
    };
    !product_path
        .split(|character: char| matches!(character, '/' | '-' | '_'))
        .any(|segment| matches!(segment, "ebook" | "kindle" | "digital" | "audible"))
}

fn search_card_marker(html: &str, cursor: usize) -> Option<(usize, usize)> {
    let markers = [
        r#"data-component-type="s-search-result""#,
        "data-component-type='s-search-result'",
        r#"cel_widget_id="MAIN-SEARCH_RESULTS-"#,
        "cel_widget_id='MAIN-SEARCH_RESULTS-",
    ];
    markers
        .iter()
        .filter_map(|marker| {
            html[cursor..]
                .find(marker)
                .map(|offset| (cursor + offset, marker.len()))
        })
        .min_by_key(|(position, _)| *position)
}

pub fn parse_amazon_search_results(html: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut cursor = 0;
    let mut found_marker = false;
    while let Some((start, marker_len)) = search_card_marker(html, cursor) {
        found_marker = true;
        let next_start = search_card_marker(html, start + marker_len)
            .map(|(next, _)| next)
            .unwrap_or(html.len());
        for href in product_hrefs(&html[start..next_start]) {
            if !results.iter().any(|value| value == &href) {
                results.push(href);
            }
        }
        cursor = start + marker_len;
    }
    if !found_marker {
        results.extend(product_hrefs(html));
    }
    results
}

pub fn parse_amazon_search_result(html: &str) -> Option<String> {
    parse_amazon_search_results(html).into_iter().next()
}

fn tag_with_id<'a>(html: &'a str, id: &str) -> Option<&'a str> {
    let double_marker = format!(r#"id="{id}""#);
    let single_marker = format!("id='{id}'");
    let position = [html.find(&double_marker), html.find(&single_marker)]
        .into_iter()
        .flatten()
        .min()?;
    let start = html[..position].rfind('<')?;
    let end = position + html[position..].find('>')? + 1;
    Some(&html[start..end])
}

fn element_content_with_id<'a>(html: &'a str, id: &str) -> Option<&'a str> {
    let tag = tag_with_id(html, id)?;
    let tag_start = html.find(tag)?;
    let name = tag
        .strip_prefix('<')?
        .split([' ', '>', '/'])
        .next()
        .filter(|name| !name.is_empty())?;
    let content_start = tag_start + tag.len();
    let closing = format!("</{name}>");
    let content_end = html[content_start..].find(&closing)? + content_start;
    Some(&html[content_start..content_end])
}

fn first_tag_content<'a>(html: &'a str, name: &str) -> Option<&'a str> {
    let opening = format!("<{name}");
    let start = html.find(&opening)?;
    let content_start = html[start..].find('>')? + start + 1;
    let closing = format!("</{name}>");
    let content_end = html[content_start..].find(&closing)? + content_start;
    Some(&html[content_start..content_end])
}

fn extract_attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let marker = format!("{name}={quote}");
        let Some(marker_start) = tag.find(&marker) else {
            continue;
        };
        let start = marker_start + marker.len();
        let end = tag[start..].find(quote).map(|offset| start + offset)?;
        let value = tag[start..end].trim();
        if !value.is_empty() {
            return Some(value.replace("&amp;", "&").replace("&quot;", "\""));
        }
    }
    None
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

fn html_text(value: &str) -> String {
    let mut text = String::with_capacity(value.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for character in value.chars() {
        match character {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                if tag.trim_start().starts_with("br") {
                    text.push('\n');
                }
            }
            _ if in_tag => tag.push(character),
            _ => text.push(character),
        }
    }
    decode_html_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn valid_image_url(value: Option<String>) -> Option<String> {
    value.filter(|url| {
        (url.starts_with("https://") || url.starts_with("http://")) && !url.starts_with("data:")
    })
}

fn extract_json_string(html: &str, marker: &str) -> Option<String> {
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find('"').map(|offset| start + offset)?;
    let value = html[start..end].replace("\\/", "/").replace("\\u0026", "&");
    (!value.is_empty()).then_some(value)
}

pub fn parse_amazon_image_url(html: &str) -> Option<String> {
    for marker in [r#""landingImageUrl":""#, r#"landingImageUrl\":\""#] {
        if let Some(url) = valid_image_url(extract_json_string(html, marker)) {
            return Some(url);
        }
    }

    for id in ["landingImage", "imgBlkFront"] {
        let Some(tag) = tag_with_id(html, id) else {
            continue;
        };
        for attribute in ["data-old-hires", "src"] {
            if let Some(url) = valid_image_url(extract_attribute(tag, attribute)) {
                return Some(url);
            }
        }
        if let Some(dynamic) = extract_attribute(tag, "data-a-dynamic-image") {
            let decoded = dynamic.replace("&quot;", "\"");
            if let Ok(images) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&decoded)
            {
                if let Some(url) = images
                    .keys()
                    .filter(|url| !url.is_empty())
                    .max_by_key(|url| url.len())
                    .cloned()
                {
                    if let Some(url) = valid_image_url(Some(url)) {
                        return Some(url);
                    }
                }
            }
        }
    }

    for marker in [
        "property=\"og:image\"",
        "property='og:image'",
        "name=\"og:image\"",
        "name='og:image'",
    ] {
        if let Some(position) = html.find(marker) {
            let start = html[..position].rfind('<')?;
            let end = position + html[position..].find('>')? + 1;
            let tag = &html[start..end];
            if let Some(url) = valid_image_url(extract_attribute(tag, "content")) {
                return Some(url);
            }
        }
    }
    None
}

pub fn parse_amazon_detail(html: &str) -> AmazonInfo {
    let title = ["productTitle", "title"]
        .iter()
        .find_map(|id| element_content_with_id(html, id).map(html_text))
        .or_else(|| {
            [
                "property=\"og:title\"",
                "property='og:title'",
                "name=\"og:title\"",
                "name='og:title'",
            ]
            .iter()
            .find_map(|marker| {
                let position = html.find(marker)?;
                let start = html[..position].rfind('<')?;
                let end = position + html[position..].find('>')? + 1;
                let value = extract_attribute(&html[start..end], "content")?;
                let value = html_text(&value);
                (!value.is_empty()).then_some(value)
            })
        });

    let cover_url = parse_amazon_image_url(html).filter(|url| !url.is_empty());
    let description = element_content_with_id(html, "bookDescription_feature_div")
        .and_then(|content| first_tag_content(content, "span"))
        .map(html_text)
        .filter(|value| !value.is_empty());
    let isbn13 = parse_amazon_isbn13(html);
    let publish_date = parse_amazon_publish_date(html);

    AmazonInfo {
        title,
        cover_url,
        description,
        publish_date,
        isbn13,
    }
}

pub fn amazon_info_matches_isbn(info: &AmazonInfo, expected_isbn13: Option<&str>) -> bool {
    expected_isbn13.is_none_or(|expected| {
        info.isbn13
            .as_deref()
            .is_none_or(|actual| actual == expected)
    })
}

pub fn amazon_info_has_expected_isbn(info: &AmazonInfo, expected_isbn13: Option<&str>) -> bool {
    expected_isbn13.is_none_or(|expected| info.isbn13.as_deref() == Some(expected))
}

pub fn amazon_metadata_is_verified(info: &AmazonInfo, expected_isbn13: Option<&str>) -> bool {
    expected_isbn13.is_none_or(|_| info.isbn13.is_some())
}

fn detail_value_after_label<'a>(html: &'a str, label: &str) -> Option<&'a str> {
    let position = html.find(label)? + label.len();
    let tail = &html[position..];
    let end = [tail.find("</li>"), tail.find("</tr>"), tail.find("</div>")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

fn clean_isbn(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit() || matches!(character, 'X' | 'x'))
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

fn parse_amazon_isbn13(html: &str) -> Option<String> {
    for (label, is_isbn10) in [("ISBN-13", false), ("ISBN-10", true)] {
        let Some(value) = detail_value_after_label(html, label) else {
            continue;
        };
        let value = html_text(value);
        for part in value.split_whitespace() {
            let candidate = clean_isbn(part);
            if is_isbn10 {
                if let Some(isbn13) = isbn10_to_isbn13(&candidate) {
                    return Some(isbn13);
                }
            } else if candidate.len() == 13
                && candidate
                    .chars()
                    .all(|character| character.is_ascii_digit())
            {
                return Some(candidate);
            }
        }
    }
    None
}

fn normalize_publish_date(value: &str) -> Option<String> {
    let value = value.trim_matches(|character: char| {
        matches!(
            character,
            ':' | '：' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}'
        )
    });
    let parts = value.split(['/', '-', '.']).collect::<Vec<_>>();
    if parts.len() != 3 || parts[0].len() != 4 {
        return None;
    }
    let year = parts[0].parse::<u32>().ok()?;
    let month = parts[1].parse::<u32>().ok()?;
    let day = parts[2].parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn parse_amazon_publish_date(html: &str) -> Option<String> {
    for label in ["発売日", "出版日"] {
        let Some(value) = detail_value_after_label(html, label) else {
            continue;
        };
        let value = html_text(value);
        for part in value.split_whitespace() {
            if let Some(date) = normalize_publish_date(part) {
                return Some(date);
            }
        }
    }
    None
}

pub fn is_allowed_amazon_url(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    host == "amazon.co.jp"
        || host.ends_with(".amazon.co.jp")
        || host == "amazon.com"
        || host.ends_with(".amazon.com")
        || host.ends_with(".media-amazon.com")
        || host.ends_with(".ssl-images-amazon.com")
}

fn amazon_asin(value: &str) -> Option<&str> {
    let path = value.split(['?', '#']).next().unwrap_or(value);
    let marker = path
        .find("/dp/")
        .map(|position| position + 4)
        .or_else(|| path.find("/gp/aw/d/").map(|position| position + 9))?;
    let asin = path[marker..].split('/').next()?;
    (asin.len() == 10 && asin.bytes().all(|byte| byte.is_ascii_alphanumeric())).then_some(asin)
}

pub fn amazon_detail_url(href: &str) -> Option<String> {
    let href = href.replace("&amp;", "&");
    let absolute = if href.starts_with('/') {
        format!("https://www.amazon.co.jp{href}")
    } else {
        href
    };
    if !is_allowed_amazon_url(&absolute) {
        return None;
    }
    let asin = amazon_asin(&absolute)?;
    Some(format!("https://www.amazon.co.jp/gp/aw/d/{asin}"))
}

pub fn amazon_isbn_detail_url(isbn: &str) -> Option<String> {
    let isbn10 = isbn_lookup_variants(isbn)
        .into_iter()
        .find(|variant| variant.len() == 10)?;
    Some(format!("https://www.amazon.co.jp/gp/aw/d/{isbn10}"))
}

pub fn needs_black_curtain_eligibility(html: &str) -> bool {
    html.contains("black-curtain-verification")
}

pub fn black_curtain_eligibility_url(detail_url: &str) -> String {
    let return_path = amazon_asin(detail_url)
        .map(|asin| format!("/dp/{asin}"))
        .unwrap_or_else(|| detail_url.to_string());
    format!(
        "https://www.amazon.co.jp/black-curtain/save-eligibility/black-curtain?returnUrl={}",
        percent_encode(&return_path)
    )
}

pub fn image_content_type(value: &str) -> Option<(&'static str, &'static str)> {
    let mime = value.split(';').next()?.trim().to_ascii_lowercase();
    match mime.as_str() {
        "image/jpeg" | "image/jpg" => Some(("image/jpeg", "jpg")),
        "image/png" => Some(("image/png", "png")),
        "image/webp" => Some(("image/webp", "webp")),
        "image/gif" => Some(("image/gif", "gif")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_native_isbn_variants() {
        assert_eq!(
            isbn13_to_isbn10("9784041164693").as_deref(),
            Some("4041164699")
        );
        assert_eq!(
            isbn10_to_isbn13("0262033844").as_deref(),
            Some("9780262033848")
        );
    }

    #[test]
    fn builds_native_search_url_first() {
        assert_eq!(
            amazon_search_url("9784041164693").as_deref(),
            Some("https://www.amazon.co.jp/s?k=9784041164693&i=stripbooks")
        );
    }

    #[test]
    fn selects_physical_product_over_ebook_variant() {
        let html = r#"
            <div data-component-type="s-search-result" data-asin="B0FX7VSWLJ">
                <a href="/secret-ebook/dp/B0FX7VSWLJ">Kindle</a>
                <a href="/secret-hardcover/dp/4041164693">Hardcover</a>
            </div>
        "#;
        assert_eq!(
            parse_amazon_search_result(html).as_deref(),
            Some("/secret-hardcover/dp/4041164693")
        );
    }

    #[test]
    fn skips_untyped_kindle_link_for_physical_book() {
        let html = r#"
            <div data-component-type="s-search-result">
                <a href="/dp/B0FX7VSWLJ">Kindle (Digital)</a>
                <a href="/dp/4041164699">Hardcover</a>
            </div>
        "#;
        assert_eq!(parse_amazon_search_results(html), vec!["/dp/4041164699"]);
    }

    #[test]
    fn canonicalizes_dp_detail_to_mobile_detail() {
        assert_eq!(
            amazon_detail_url("/secret-hardcover/dp/4041164699/ref=sr_1_1").as_deref(),
            Some("https://www.amazon.co.jp/gp/aw/d/4041164699")
        );
    }

    #[test]
    fn builds_isbn_detail_fallback() {
        assert_eq!(
            amazon_isbn_detail_url("9784041164693").as_deref(),
            Some("https://www.amazon.co.jp/gp/aw/d/4041164699")
        );
    }

    #[test]
    fn parses_r18_detail_and_cover_fallbacks() {
        let html = r#"
            <div id="black-curtain-verification"></div>
            <span id="productTitle">Example &amp; Title</span>
            <img id="imgBlkFront" src="https://m.media-amazon.com/images/I/cover.jpg">
            <div id="bookDescription_feature_div"><span>Amazon description</span></div>
            <ul id="detailBullets_feature_div">
                <li>発売日 : 2015/1/26</li>
                <li>ISBN-13 : 978-4569823522</li>
            </ul>
        "#;
        let info = parse_amazon_detail(html);
        assert_eq!(info.title.as_deref(), Some("Example & Title"));
        assert_eq!(
            info.cover_url.as_deref(),
            Some("https://m.media-amazon.com/images/I/cover.jpg")
        );
        assert!(!amazon_info_has_expected_isbn(
            &AmazonInfo::default(),
            Some("9784569823522")
        ));
        assert_eq!(info.description.as_deref(), Some("Amazon description"));
        assert_eq!(info.publish_date.as_deref(), Some("2015-01-26"));
        assert_eq!(info.isbn13.as_deref(), Some("9784569823522"));
        assert!(needs_black_curtain_eligibility(html));
    }

    #[test]
    fn validates_amazon_metadata_against_expected_isbn() {
        let info = AmazonInfo {
            isbn13: Some("9784569823522".to_string()),
            ..AmazonInfo::default()
        };
        assert!(amazon_info_matches_isbn(&info, Some("9784569823522")));
        assert!(!amazon_info_matches_isbn(&info, Some("9784569823521")));
        assert!(!amazon_metadata_is_verified(
            &AmazonInfo::default(),
            Some("9784569823522")
        ));
        assert!(amazon_metadata_is_verified(&info, Some("9784569823522")));
    }

    #[test]
    fn builds_black_curtain_eligibility_url_from_detail_path() {
        assert_eq!(
            black_curtain_eligibility_url("https://www.amazon.co.jp/book/dp/B000000001/ref=x"),
            "https://www.amazon.co.jp/black-curtain/save-eligibility/black-curtain?returnUrl=%2Fdp%2FB000000001"
        );
    }
}
