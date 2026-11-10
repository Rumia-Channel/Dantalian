//! narou.rs import payloads and tag selection shared by Native and Worker.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

pub const KEY_URL: &str = "narou.url";
pub const IMPORT_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS narou_imports (
    source_url TEXT NOT NULL,
    novel_id INTEGER NOT NULL,
    book_id INTEGER NOT NULL UNIQUE REFERENCES books(id) ON DELETE CASCADE,
    file_hash TEXT NOT NULL,
    remote_version TEXT NOT NULL DEFAULT '',
    active INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (source_url, novel_id)
);";

// Run together in one transaction/batch. Parameters: source, novel id, title,
// publisher, description, author, hash, filename, remote version. Conditional creation also
// protects against overlapping sync requests without an application lock.
pub const IMPORT_SQL: &[(&str, usize)] = &[
    ("INSERT INTO books(title,publisher,description,media_type,created_at,updated_at)
     SELECT ?3,?4,?5,'book',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP
     WHERE NOT EXISTS(SELECT 1 FROM narou_imports WHERE source_url=?1 AND novel_id=?2)", 5),
    ("INSERT INTO narou_imports(source_url,novel_id,book_id,file_hash)
     SELECT ?1,?2,last_insert_rowid(),?7
     WHERE NOT EXISTS(SELECT 1 FROM narou_imports WHERE source_url=?1 AND novel_id=?2)", 7),
    ("UPDATE books SET title=?3,publisher=?4,description=?5,updated_at=CURRENT_TIMESTAMP
     WHERE id=(SELECT book_id FROM narou_imports WHERE source_url=?1 AND novel_id=?2)", 5),
    ("DELETE FROM book_files WHERE book_id=(SELECT book_id FROM narou_imports WHERE source_url=?1 AND novel_id=?2)
     AND file_hash=(SELECT file_hash FROM narou_imports WHERE source_url=?1 AND novel_id=?2) AND file_hash<>?7", 7),
    ("INSERT INTO book_files(book_id,file_hash,file_name,created_at)
     SELECT book_id,?7,?8,CURRENT_TIMESTAMP FROM narou_imports WHERE source_url=?1 AND novel_id=?2
     ON CONFLICT(book_id,file_hash) DO UPDATE SET file_name=excluded.file_name", 8),
    ("UPDATE narou_imports SET file_hash=?7,remote_version=?9,active=1 WHERE source_url=?1 AND novel_id=?2", 9),
    ("DELETE FROM book_authors WHERE book_id=(SELECT book_id FROM narou_imports WHERE source_url=?1 AND novel_id=?2)", 2),
    ("INSERT INTO authors(name) SELECT ?6 WHERE ?6<>'' AND NOT EXISTS(SELECT 1 FROM authors WHERE name=?6)", 6),
    ("INSERT INTO book_authors(book_id,author_id,sort_order)
     SELECT book_id,(SELECT MIN(id) FROM authors WHERE name=?6),0 FROM narou_imports
     WHERE source_url=?1 AND novel_id=?2 AND ?6<>''", 6),
];

pub const MAPPING_SQL: &str =
    "SELECT book_id,file_hash FROM narou_imports WHERE source_url=?1 AND novel_id=?2";

#[derive(Debug, Deserialize)]
pub struct ImportMapping {
    pub book_id: i64,
    pub file_hash: String,
}

pub const CACHE_SQL: &str = "SELECT n.book_id,n.file_hash,n.remote_version FROM narou_imports n
    JOIN book_files f ON f.book_id=n.book_id AND f.file_hash=n.file_hash
    WHERE n.source_url=?1 AND n.novel_id=?2";
pub const RECONCILE_SQL: &str = "UPDATE narou_imports SET active=(source_url=?1 AND novel_id IN (SELECT value FROM json_each(?2)))";

#[derive(Debug, Deserialize)]
pub struct CachedNovel {
    pub book_id: i64,
    pub file_hash: String,
    pub remote_version: String,
}

/// EPUB requires the first ZIP entry to be the uncompressed mimetype file.
pub fn validate_epub(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() >= 58
        && bytes.starts_with(b"PK\x03\x04")
        && bytes[8..10] == [0, 0]
        && bytes[26..28] == [8, 0]
    {
        let extra = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
        if bytes.get(30..38) == Some(b"mimetype")
            && bytes.get(38 + extra..58 + extra) == Some(b"application/epub+zip")
        {
            return Ok(());
        }
    }
    Err("narou.rs から有効な EPUB を取得できませんでした（EPUB 変換設定を確認してください）".into())
}
pub const KEY_TOKEN: &str = "narou.token";
pub const KEY_TAGS: &str = "narou.tags";
pub const KEY_FAVORITES: &str = "narou.use_favorites";
pub const KEY_CF_ID: &str = "narou.cf_access_client_id";
pub const KEY_CF_SECRET: &str = "narou.cf_access_client_secret";

pub fn validate_auth_settings(settings: &HashMap<String, String>) -> Result<(), String> {
    for key in [KEY_TOKEN, KEY_CF_ID, KEY_CF_SECRET] {
        if settings
            .get(key)
            .is_some_and(|value| !value.bytes().all(|byte| (b'!'..=b'~').contains(&byte)))
        {
            return Err(format!(
                "{key} は空白・制御文字を含まない ASCII 文字列にしてください"
            ));
        }
    }
    let configured = |key: &str| settings.get(key).is_some_and(|value| !value.is_empty());
    if configured(KEY_CF_ID) != configured(KEY_CF_SECRET) {
        return Err(
            "Cloudflare Access の Client ID と Client Secret は両方設定してください".into(),
        );
    }
    Ok(())
}

pub struct Config {
    pub url: String,
    pub token: String,
    pub cf_access_client_id: String,
    pub cf_access_client_secret: String,
    pub tags: Vec<String>,
    pub use_favorites: bool,
}

impl Config {
    pub fn from_settings(settings: &HashMap<String, String>) -> Result<Self, String> {
        validate_auth_settings(settings)?;
        let url = settings
            .get(KEY_URL)
            .map(|s| s.trim().trim_end_matches('/'))
            .unwrap_or("");
        if url.is_empty() {
            return Err("narou.rs の接続先 URL を設定してください".into());
        }
        Ok(Self {
            url: url.to_owned(),
            token: settings.get(KEY_TOKEN).cloned().unwrap_or_default(),
            cf_access_client_id: settings.get(KEY_CF_ID).cloned().unwrap_or_default(),
            cf_access_client_secret: settings.get(KEY_CF_SECRET).cloned().unwrap_or_default(),
            tags: parse_tags(settings.get(KEY_TAGS).map(String::as_str).unwrap_or("[]"))?,
            use_favorites: settings
                .get(KEY_FAVORITES)
                .map(String::as_str)
                .unwrap_or("true")
                == "true",
        })
    }

    pub fn has_credentials(&self) -> bool {
        !self.token.is_empty() || !self.cf_access_client_id.is_empty()
    }

    pub fn selected_tags(&self, favorites: &[String]) -> BTreeSet<String> {
        self.tags
            .iter()
            .chain(favorites.iter().filter(|_| self.use_favorites))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }
}

pub fn parse_tags(value: &str) -> Result<Vec<String>, String> {
    let tags: Vec<String> = serde_json::from_str(value)
        .map_err(|_| "narou.tags はタグ名の JSON 配列で指定してください".to_string())?;
    if tags
        .iter()
        .any(|tag| tag.trim().is_empty() || tag.chars().any(char::is_control))
    {
        return Err("空のタグ名や制御文字は指定できません".into());
    }
    Ok(tags.into_iter().map(|tag| tag.trim().to_owned()).collect())
}

#[derive(Debug, Deserialize)]
pub struct FavoriteTags {
    pub success: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NovelList {
    pub data: Vec<Novel>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Novel {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub last_update: i64,
    pub general_lastup: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub sitename: String,
    pub display_url: Option<String>,
    pub toc_url: Option<String>,
}

impl Novel {
    pub fn matches(&self, tags: &BTreeSet<String>) -> bool {
        self.tags.iter().any(|tag| tags.contains(tag))
    }

    pub fn description(&self) -> String {
        format!(
            "{}\nタグ: {}",
            self.display_url
                .as_deref()
                .or(self.toc_url.as_deref())
                .unwrap_or(""),
            self.tags.join("、")
        )
    }
}

#[derive(Debug, Default, Serialize)]
pub struct SyncResult {
    pub matched: usize,
    pub imported: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub tags: Vec<String>,
    pub book_ids: Vec<i64>,
    pub failed: Vec<SyncFailure>,
}

#[derive(Debug, Serialize)]
pub struct SyncFailure {
    pub novel_id: i64,
    pub title: String,
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_union_is_exact_and_empty_selection_imports_nothing() {
        let mut config = Config::from_settings(&HashMap::from([
            (KEY_URL.into(), "http://127.0.0.1:8000/".into()),
            (KEY_TAGS.into(), r#"["指定", "共通"]"#.into()),
        ]))
        .unwrap();
        let favorites = vec!["お気に入り".into(), "共通".into()];
        let selected = config.selected_tags(&favorites);
        assert_eq!(
            selected,
            BTreeSet::from(["指定".into(), "共通".into(), "お気に入り".into()])
        );
        let mut novel = Novel {
            id: 1,
            title: "作品".into(),
            author: "著者".into(),
            tags: vec!["指定の続き".into()],
            sitename: String::new(),
            display_url: None,
            toc_url: None,
            last_update: 1,
            general_lastup: None,
        };
        assert!(!novel.matches(&selected));
        novel.tags = vec!["お気に入り".into()];
        assert!(novel.matches(&selected));
        config.use_favorites = false;
        assert!(!novel.matches(&config.selected_tags(&favorites)));
        config.tags.clear();
        assert!(!novel.matches(&config.selected_tags(&favorites)));
    }

    #[test]
    fn access_credentials_require_a_complete_header_safe_pair() {
        let mut settings = HashMap::from([(KEY_CF_ID.into(), "client.access".into())]);
        assert!(validate_auth_settings(&settings).is_err());
        settings.insert(KEY_CF_SECRET.into(), "cfast_abc-123".into());
        assert!(validate_auth_settings(&settings).is_ok());
        settings.insert(KEY_TOKEN.into(), "injected\r\nHeader: value".into());
        assert!(validate_auth_settings(&settings).is_err());
        settings.insert(KEY_TOKEN.into(), "bearer-token".into());
        assert!(validate_auth_settings(&settings).is_ok());
    }
}
