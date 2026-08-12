use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Author {
    pub id: i64,
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CreateAuthor {
    pub name: String,
    pub transcription: Option<String>,
    pub ndl_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UpdateAuthor {
    pub name: String,
    pub transcription: Option<String>,
    pub ndl_id: Option<String>,
}
