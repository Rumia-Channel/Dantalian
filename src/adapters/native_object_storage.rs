use std::path::PathBuf;

use crate::{
    application::error::AppError,
    ports::object_storage::{ObjectMetadata, ObjectStorage},
};

#[derive(Debug, Clone)]
pub struct NativeObjectStorage {
    root: PathBuf,
    public_base_url: String,
}

impl NativeObjectStorage {
    pub fn new(root: impl Into<PathBuf>, public_base_url: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            public_base_url: public_base_url.into(),
        }
    }

    fn path_for(&self, key: &str) -> Result<PathBuf, AppError> {
        validate_key(key)?;
        Ok(key
            .split('/')
            .fold(self.root.clone(), |path, component| path.join(component)))
    }
}

impl ObjectStorage for NativeObjectStorage {
    async fn head(&self, key: &str) -> Result<ObjectMetadata, AppError> {
        let metadata = tokio::fs::metadata(self.path_for(key)?)
            .await
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    AppError::NotFound
                } else {
                    storage_error(error)
                }
            })?;
        Ok(ObjectMetadata {
            content_length: Some(metadata.len()),
            content_type: None,
        })
    }

    async fn exists(&self, key: &str) -> Result<bool, AppError> {
        match self.head(key).await {
            Ok(_) => Ok(true),
            Err(AppError::NotFound) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn put_object(
        &self,
        key: &str,
        _content_type: &str,
        bytes: &[u8],
    ) -> Result<(), AppError> {
        let path = self.path_for(key)?;
        let parent = path
            .parent()
            .ok_or_else(|| AppError::Storage("object path has no parent".to_string()))?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(storage_error)?;
        tokio::fs::write(path, bytes).await.map_err(storage_error)
    }

    async fn delete(&self, key: &str) -> Result<(), AppError> {
        let path = self.path_for(key)?;
        tokio::fs::remove_file(path).await.map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AppError::NotFound
            } else {
                storage_error(error)
            }
        })
    }

    async fn temporary_get_url(&self, key: &str) -> Result<String, AppError> {
        validate_key(key)?;
        let base = self.public_base_url.trim_end_matches('/');
        Ok(if base.is_empty() {
            format!("/{key}")
        } else {
            format!("{base}/{key}")
        })
    }
}

fn validate_key(key: &str) -> Result<(), AppError> {
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

fn storage_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_before_touching_filesystem() {
        let storage = NativeObjectStorage::new(std::path::Path::new("."), "");
        assert!(storage.path_for("../secret").is_err());
        assert!(storage.path_for("audio\\secret").is_err());
    }

    #[tokio::test]
    async fn stores_deletes_and_builds_public_urls() {
        let root =
            std::env::temp_dir().join(format!("dantalian-object-storage-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&root).await;
        let storage = NativeObjectStorage::new(&root, "https://example.test/media");

        storage
            .put_object("images/cover.jpg", "image/jpeg", b"cover")
            .await
            .expect("object should be written");
        assert!(
            storage
                .exists("images/cover.jpg")
                .await
                .expect("object lookup")
        );
        assert_eq!(
            storage
                .temporary_get_url("images/cover.jpg")
                .await
                .expect("public URL"),
            "https://example.test/media/images/cover.jpg"
        );
        storage
            .delete("images/cover.jpg")
            .await
            .expect("object should be deleted");
        assert!(
            !storage
                .exists("images/cover.jpg")
                .await
                .expect("object lookup")
        );
        assert!(matches!(
            storage.delete("images/cover.jpg").await,
            Err(AppError::NotFound)
        ));

        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
