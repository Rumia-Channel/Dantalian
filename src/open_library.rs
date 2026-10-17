use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use std::borrow::Cow;

pub const SEARCH_ENDPOINT: &str = "https://openlibrary.org/search.json";
// The parent key is required by Open Library's edition subquery, even though we
// do not store it. Omitting it produces works with empty edition results.
pub const SEARCH_FIELDS: &str = "key,author_name,editions,editions.key,editions.title,editions.publisher,editions.publish_date,editions.isbn";
pub const SEARCH_LIMIT: &str = "5";
const USER_AGENT: &str = concat!(
    "Dantalian/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Rumia-Channel/Dantalian)"
);

pub struct Book {
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub authors: Vec<String>,
}

pub fn user_agent(contact: Option<&str>) -> Cow<'static, str> {
    match contact.map(str::trim).filter(|value| !value.is_empty()) {
        Some(contact) => Cow::Owned(format!("{USER_AGENT} ({contact})")),
        None => Cow::Borrowed(USER_AGENT),
    }
}

pub fn search_query(isbn_variants: &[String]) -> String {
    let mut query = String::with_capacity(
        7 + isbn_variants
            .iter()
            .map(|isbn| isbn.len() + 6)
            .sum::<usize>(),
    );
    query.push_str("isbn:(");
    for (index, isbn) in isbn_variants.iter().enumerate() {
        if index != 0 {
            query.push_str(" OR ");
        }
        query.push('"');
        for character in isbn.chars() {
            if matches!(character, '\\' | '"') {
                query.push('\\');
            }
            query.push(character);
        }
        query.push('"');
    }
    query.push(')');
    query
}

#[derive(Deserialize)]
struct SearchResults {
    docs: Vec<Work>,
}

#[derive(Deserialize)]
struct Work {
    #[serde(default)]
    author_name: Vec<String>,
    editions: Option<Editions>,
}

#[derive(Deserialize)]
struct Editions {
    docs: Vec<Edition>,
}

#[derive(Deserialize)]
struct Edition {
    title: Option<String>,
    #[serde(default)]
    isbn: Vec<String>,
    #[serde(default)]
    publisher: Vec<String>,
    #[serde(default)]
    publish_date: Vec<String>,
}

pub fn parse_search_results(json: &[u8], isbn_variants: &[String]) -> Result<Option<Book>, String> {
    let results: SearchResults = serde_json::from_slice(json)
        .map_err(|error| format!("Open Library response parse failed: {error}"))?;
    for mut work in results.docs {
        let Some(editions) = work.editions else {
            continue;
        };
        for edition in editions.docs {
            // Work-level ISBNs, publishers and dates span unrelated editions.
            // Never attach their metadata to the requested ISBN.
            if !edition.isbn.iter().any(|isbn| isbn_variants.contains(isbn)) {
                continue;
            }
            let Some(title) = edition.title.filter(|title| !title.trim().is_empty()) else {
                continue;
            };
            work.author_name.retain(|name| !name.trim().is_empty());
            return Ok(Some(Book {
                title,
                publisher: edition
                    .publisher
                    .into_iter()
                    .find(|name| !name.trim().is_empty()),
                publish_date: edition
                    .publish_date
                    .iter()
                    .find_map(|date| normalize_date(date)),
                authors: work.author_name,
            }));
        }
    }
    Ok(None)
}

fn normalize_date(raw: &str) -> Option<String> {
    if let Some(date) = crate::application::publish_date::normalize_publish_date(Some(raw)) {
        return Some(date);
    }
    let raw = raw.trim();
    for format in ["%B %e, %Y", "%b %e, %Y", "%e %B %Y", "%e %b %Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(raw, format) {
            return (1900..=2999)
                .contains(&date.year())
                .then(|| date.format("%Y-%m-%d").to_string());
        }
    }
    for format in ["%B %Y", "%b %Y"] {
        let mut parsed = chrono::format::Parsed::new();
        if chrono::format::parse(&mut parsed, raw, chrono::format::StrftimeItems::new(format))
            .is_ok()
            && parsed.set_day(1).is_ok()
        {
            let date = parsed.to_naive_date().ok()?;
            return (1900..=2999)
                .contains(&date.year())
                .then(|| date.format("%Y-%m-NN").to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests;
