use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::application::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    CoverImage,
    Epub,
    OriginalAudio,
    EncodedAudio { codec: AudioCodec },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    Opus,
    Aac,
}

impl AudioCodec {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opus => "opus",
            Self::Aac => "aac",
        }
    }
}
/// Maximum request body that the Worker handles directly before the client
/// must switch to a Wasabi multipart upload.
pub const WORKER_DIRECT_UPLOAD_MAX_BYTES: u64 = 95 * 1024 * 1024;

/// Fixed part size for browser-to-Wasabi multipart uploads.
pub const MULTIPART_PART_SIZE: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartObjectMetadata {
    pub content_length: u64,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartPart {
    pub part_number: u32,
    pub etag: String,
}

pub trait MultipartUploadStorage {
    fn create_multipart_upload(
        &self,
        key: &str,
        content_type: &str,
    ) -> impl Future<Output = Result<String, AppError>>;
    fn presigned_upload_part_url(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
    ) -> Result<String, AppError>;
    fn complete_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
        parts: &[MultipartPart],
    ) -> impl Future<Output = Result<MultipartObjectMetadata, AppError>>;
    fn abort_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
    ) -> impl Future<Output = Result<(), AppError>>;
}

pub trait ObjectStorage {
    fn exists(&self, key: &str) -> impl Future<Output = Result<bool, AppError>>;
    fn put_object(
        &self,
        key: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> impl Future<Output = Result<(), AppError>>;
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
        ObjectKind::OriginalAudio => format!("audio/{object_id}.{extension}"),
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

pub fn validate_object_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() || key.starts_with('/') || key.ends_with('/') || key.contains('\\') {
        return Err(AppError::Validation("Invalid object key".to_string()));
    }
    if !key.split('/').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    }) {
        return Err(AppError::Validation("Invalid object key".to_string()));
    }
    Ok(())
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

    #[test]
    fn validates_object_keys_without_allowing_traversal() {
        assert!(validate_object_key("audio/original.mp3").is_ok());
        assert!(validate_object_key("audio/../secret.mp3").is_err());
        assert!(validate_object_key("/audio/file.mp3").is_err());
    }
}
