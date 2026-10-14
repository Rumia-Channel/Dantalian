use serde::{Deserialize, Serialize};

/// One SQLite/D1 snapshot: only the current identity and immediate neighbors are read.
pub const NAVIGATION_QUERY: &str = r#"
SELECT b.id, b.media_type, b.epub_file_hash, b.epub_file_name,
       s.id AS series_id, s.name AS series_name,
       p.id AS previous_id, p.title AS previous_title,
       p.epub_file_hash AS previous_hash, p.epub_file_name AS previous_file_name,
       n.id AS next_id, n.title AS next_title,
       n.epub_file_hash AS next_hash, n.epub_file_name AS next_file_name
FROM books b
LEFT JOIN series s ON s.id = b.series_id
LEFT JOIN books p ON s.id IS NOT NULL AND b.epub_file_hash = ?2
    AND COALESCE(b.media_type, 'book') = 'book' AND p.id = (
    SELECT candidate.id FROM books candidate
    WHERE candidate.series_id = b.series_id
      AND COALESCE(candidate.media_type, 'book') = 'book'
      AND (candidate.series_number IS NULL, COALESCE(candidate.series_number, 0), candidate.id)
        < (b.series_number IS NULL, COALESCE(b.series_number, 0), b.id)
    ORDER BY candidate.series_number IS NULL DESC, candidate.series_number DESC, candidate.id DESC
    LIMIT 1
)
LEFT JOIN books n ON s.id IS NOT NULL AND b.epub_file_hash = ?2
    AND COALESCE(b.media_type, 'book') = 'book' AND n.id = (
    SELECT candidate.id FROM books candidate
    WHERE candidate.series_id = b.series_id
      AND COALESCE(candidate.media_type, 'book') = 'book'
      AND (candidate.series_number IS NULL, COALESCE(candidate.series_number, 0), candidate.id)
        > (b.series_number IS NULL, COALESCE(b.series_number, 0), b.id)
    ORDER BY candidate.series_number IS NULL, candidate.series_number, candidate.id
    LIMIT 1
)
WHERE b.id = ?1
"#;

#[derive(Debug, Deserialize)]
pub struct NavigationQuery {
    pub book_key: Option<String>,
    pub content_hash: Option<String>,
    pub return_to: Option<String>,
}

pub struct NavigationRequest {
    pub id: i64,
    pub book_key: String,
    pub content_hash: String,
    pub return_to: Option<String>,
}

impl NavigationQuery {
    pub fn validate(self) -> Result<NavigationRequest, NavigationError> {
        let book_key = self.book_key.ok_or(NavigationError::InvalidRequest)?;
        let raw = book_key
            .strip_prefix("book:")
            .ok_or(NavigationError::InvalidRequest)?;
        if raw.is_empty() || raw.starts_with('0') || !raw.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(NavigationError::InvalidRequest);
        }
        let id = raw
            .parse::<i64>()
            .map_err(|_| NavigationError::InvalidRequest)?;
        let content_hash = self
            .content_hash
            .filter(|value| !value.is_empty())
            .ok_or(NavigationError::InvalidRequest)?;
        Ok(NavigationRequest {
            id,
            book_key,
            content_hash,
            return_to: self.return_to,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct NavigationRow {
    pub id: i64,
    pub media_type: Option<String>,
    pub epub_file_hash: Option<String>,
    pub epub_file_name: Option<String>,
    pub series_id: Option<i64>,
    pub series_name: Option<String>,
    pub previous_id: Option<i64>,
    pub previous_title: Option<String>,
    pub previous_hash: Option<String>,
    pub previous_file_name: Option<String>,
    pub next_id: Option<i64>,
    pub next_title: Option<String>,
    pub next_hash: Option<String>,
    pub next_file_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationContext {
    pub schema_version: u8,
    pub book_key: String,
    pub content_hash: String,
    pub series: Option<NavigationSeries>,
    pub previous: Option<NavigationNeighbor>,
    pub next: Option<NavigationNeighbor>,
    pub return_to: NavigationReturn,
}

#[derive(Debug, Serialize)]
pub struct NavigationSeries {
    pub id: i64,
    pub name: String,
    pub order: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationNeighbor {
    pub book_key: String,
    pub title: String,
    pub content_hash: Option<String>,
    pub status: &'static str,
    pub href: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NavigationReturn {
    pub href: String,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationError {
    InvalidRequest,
    BookNotFound,
    ContentChanged,
    BookUnavailable,
    Database,
}

#[derive(Serialize)]
pub struct NavigationErrorBody {
    pub error: &'static str,
    pub code: &'static str,
}

impl NavigationError {
    pub fn status(self) -> u16 {
        match self {
            Self::InvalidRequest => 400,
            Self::BookNotFound => 404,
            Self::ContentChanged => 409,
            Self::BookUnavailable => 422,
            Self::Database => 500,
        }
    }

    pub fn body(self) -> NavigationErrorBody {
        let (code, error) = match self {
            Self::InvalidRequest => (
                "INVALID_REQUEST",
                "読書用の書籍キー・ファイル版が正しくありません。",
            ),
            Self::BookNotFound => ("BOOK_NOT_FOUND", "本が見つかりません。"),
            Self::ContentChanged => (
                "CONTENT_CHANGED",
                "読書中のファイルが変更されています。開き直してください。",
            ),
            Self::BookUnavailable => (
                "BOOK_UNAVAILABLE",
                "この本には対応する EPUB が登録されていません。",
            ),
            Self::Database => ("DATABASE_ERROR", "読書の移動先を取得できませんでした。"),
        };
        NavigationErrorBody { error, code }
    }
}

pub fn safe_return_path(value: Option<&str>) -> &str {
    let Some(value) = value else {
        return "/books/";
    };
    if value.starts_with("//")
        || value
            .bytes()
            .any(|byte| byte == b'\\' || byte.is_ascii_control())
    {
        return "/books/";
    }
    let path = value.split(['?', '#']).next().unwrap_or(value);
    if matches!(path, "/" | "/books/" | "/edit/") {
        value
    } else {
        "/books/"
    }
}

fn encode_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    encoded
}

fn file_status(hash: Option<&str>, name: Option<&str>) -> &'static str {
    if hash.is_none_or(str::is_empty) {
        return "missing-file";
    }
    if let Some((_, extension)) = name.and_then(|value| value.rsplit_once('.')) {
        if !extension.eq_ignore_ascii_case("epub") && !extension.eq_ignore_ascii_case("zip") {
            return "unsupported-format";
        }
    }
    "ready"
}

fn neighbor(
    id: Option<i64>,
    title: Option<String>,
    hash: Option<String>,
    file_name: Option<String>,
    entry: &str,
    return_path: &str,
    encoded_return: &mut Option<String>,
) -> Result<Option<NavigationNeighbor>, NavigationError> {
    let Some(id) = id else {
        return Ok(None);
    };
    let title = title.ok_or(NavigationError::Database)?;
    let status = file_status(hash.as_deref(), file_name.as_deref());
    let href = (status == "ready").then(|| {
        let origin = encoded_return.get_or_insert_with(|| encode_component(return_path));
        format!("/reader/?book={id}&entry={entry}&from={origin}")
    });
    Ok(Some(NavigationNeighbor {
        book_key: format!("book:{id}"),
        title,
        content_hash: hash,
        status,
        href,
    }))
}

pub fn resolve(
    row: Option<NavigationRow>,
    request: NavigationRequest,
) -> Result<NavigationContext, NavigationError> {
    let row = row.ok_or(NavigationError::BookNotFound)?;
    if row.id != request.id {
        return Err(NavigationError::Database);
    }
    if row
        .media_type
        .as_deref()
        .is_some_and(|value| value != "book")
        || file_status(row.epub_file_hash.as_deref(), row.epub_file_name.as_deref()) != "ready"
    {
        return Err(NavigationError::BookUnavailable);
    }
    if row.epub_file_hash.as_deref() != Some(request.content_hash.as_str()) {
        return Err(NavigationError::ContentChanged);
    }
    let return_path = safe_return_path(request.return_to.as_deref());
    let mut encoded_return = None;
    let series = row
        .series_id
        .map(|id| -> Result<_, NavigationError> {
            Ok(NavigationSeries {
                id,
                name: row.series_name.ok_or(NavigationError::Database)?,
                order: "series-number-asc-id-asc",
            })
        })
        .transpose()?;
    let previous = neighbor(
        row.previous_id,
        row.previous_title,
        row.previous_hash,
        row.previous_file_name,
        "end",
        return_path,
        &mut encoded_return,
    )?;
    let next = neighbor(
        row.next_id,
        row.next_title,
        row.next_hash,
        row.next_file_name,
        "start",
        return_path,
        &mut encoded_return,
    )?;
    Ok(NavigationContext {
        schema_version: 1,
        book_key: request.book_key,
        content_hash: request.content_hash,
        series,
        previous,
        next,
        return_to: NavigationReturn {
            href: return_path.to_owned(),
            label: "元のページへ戻る",
        },
    })
}

#[cfg(test)]
mod tests;
