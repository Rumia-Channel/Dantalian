use std::future::Future;

use crate::application::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    CoverImage,
    Epub,
    OriginalAudio,
    EncodedAudio { codec: AudioCodec },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Opus,
    Aac,
}

pub trait ObjectStorage {
    fn exists(&self, key: &str) -> impl Future<Output = Result<bool, AppError>>;
    fn delete(&self, key: &str) -> impl Future<Output = Result<(), AppError>>;
    fn temporary_get_url(&self, key: &str) -> impl Future<Output = Result<String, AppError>>;
}

pub fn object_key(
    prefix: Option<&str>,
    kind: ObjectKind,
    object_id: &str,
    extension: &str,
) -> Result<String, AppError> {
    validate_component(object_id, "object id")?;
    let extension = validate_component(extension, "object extension")?.to_ascii_lowercase();
    let path = match kind {
        ObjectKind::CoverImage => format!("images/{object_id}.{extension}"),
        ObjectKind::Epub => format!("epubs/{object_id}.{extension}"),
        ObjectKind::OriginalAudio => format!("audio/original/{object_id}.{extension}"),
        ObjectKind::EncodedAudio {
            codec: AudioCodec::Opus,
        } => {
            format!("audio/encoded/opus/{object_id}.opus")
        }
        ObjectKind::EncodedAudio {
            codec: AudioCodec::Aac,
        } => {
            format!("audio/encoded/aac/{object_id}.aac")
        }
    };
    let prefix = prefix
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
        .map(|prefix| prefix.trim_matches('/'));
    if let Some(prefix) = prefix {
        if prefix.is_empty()
            || !prefix
                .split('/')
                .all(|component| !component.is_empty() && is_safe_component(component))
        {
            return Err(AppError::Validation("Invalid object prefix".to_string()));
        }
    }
    Ok(match prefix {
        Some(prefix) => format!("{prefix}/{path}"),
        None => path,
    })
}

fn is_safe_component(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_component<'a>(value: &'a str, name: &str) -> Result<&'a str, AppError> {
    if value.is_empty() || !is_safe_component(value) {
        return Err(AppError::Validation(format!("Invalid {name}")));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_prefixed_keys_for_supported_objects() {
        assert_eq!(
            object_key(Some("production/"), ObjectKind::CoverImage, "abc123", "JPG").unwrap(),
            "production/images/abc123.jpg"
        );
        assert_eq!(
            object_key(
                None,
                ObjectKind::EncodedAudio {
                    codec: AudioCodec::Opus
                },
                "abc123",
                "ignored"
            )
            .unwrap(),
            "audio/encoded/opus/abc123.opus"
        );
    }

    #[test]
    fn rejects_path_traversal_components() {
        assert!(object_key(None, ObjectKind::Epub, "../secret", "epub").is_err());
        assert!(object_key(None, ObjectKind::Epub, "book", "../epub").is_err());
    }
}
