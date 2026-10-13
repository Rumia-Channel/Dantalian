use serde::{Deserialize, Deserializer};

/// Distinguish a missing update field from an explicit JSON null.
/// Use with `#[serde(default, deserialize_with = "deserialize_nullable")]`.
pub fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}
