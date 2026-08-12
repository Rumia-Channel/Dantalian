use serde::{Deserialize, Serialize};

use super::author::Author;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookSummary {
    pub id: i64,
    pub isbn: Option<String>,
    pub isdn: Option<String>,
    pub jan: Option<String>,
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub series_id: Option<i64>,
    pub series_number: Option<i64>,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookDetail {
    #[serde(flatten)]
    pub book: BookSummary,
    pub authors: Vec<Author>,
}
