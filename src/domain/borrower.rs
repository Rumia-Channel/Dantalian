use super::patch::deserialize_nullable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Borrower {
    pub id: i64,
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CreateBorrower {
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UpdateBorrower {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub notes: Option<Option<String>>,
}
