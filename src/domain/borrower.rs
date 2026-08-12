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
    pub notes: Option<String>,
}
