//! Connection information for browser-direct, user-authenticated narou.rs reads.
use serde::Serialize;
use std::collections::HashMap;

pub const KEY_URL: &str = "narou.url";
pub const KEY_TAGS: &str = "narou.tags";
pub const KEY_FAVORITES: &str = "narou.use_favorites";
pub const READ_API_PATH: &str = "/api/read/v1";

pub struct Config {
    pub url: String,
    pub tags: Vec<String>,
    pub use_favorites: bool,
}

impl Config {
    pub fn from_settings(settings: &HashMap<String, String>) -> Result<Self, String> {
        Ok(Self {
            url: settings
                .get(KEY_URL)
                .map(|value| value.trim().trim_end_matches('/'))
                .unwrap_or("")
                .to_owned(),
            tags: parse_tags(settings.get(KEY_TAGS).map(String::as_str).unwrap_or("[]"))?,
            use_favorites: settings
                .get(KEY_FAVORITES)
                .map(String::as_str)
                .unwrap_or("true")
                == "true",
        })
    }

    pub fn into_connection(self) -> Connection {
        let configured = !self.url.is_empty();
        let api_base = if configured {
            format!("{}{READ_API_PATH}", self.url)
        } else {
            String::new()
        };
        Connection {
            configured,
            source: self.url,
            api_base,
            tags: self.tags,
            use_favorites: self.use_favorites,
        }
    }
}

#[derive(Serialize)]
pub struct Connection {
    pub configured: bool,
    pub source: String,
    pub api_base: String,
    pub tags: Vec<String>,
    pub use_favorites: bool,
}

pub fn parse_tags(value: &str) -> Result<Vec<String>, String> {
    let mut tags: Vec<String> = serde_json::from_str(value)
        .map_err(|_| "narou.tags はタグ名の JSON 配列で指定してください".to_string())?;
    for tag in &mut tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() || tag.chars().any(char::is_control) {
            return Err("空のタグ名や制御文字は指定できません".into());
        }
        if trimmed.len() != tag.len() {
            *tag = trimmed.to_owned();
        }
    }
    Ok(tags)
}
