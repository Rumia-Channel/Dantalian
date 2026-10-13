use super::patch::deserialize_nullable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageLocation {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CreateStorageLocation {
    pub name: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UpdateStorageLocation {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_nullable")]
    pub parent_id: Option<Option<i64>>,
}
