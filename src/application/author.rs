use super::error::AppError;
use crate::{domain::author::Author, ports::author_repository::AuthorRepository};

pub struct AuthorService<R> {
    repo: R,
}

impl<R> AuthorService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R: AuthorRepository> AuthorService<R> {
    pub async fn list(&self) -> Result<Vec<Author>, AppError> {
        self.repo.list().await
    }

    pub async fn get(&self, id: i64) -> Result<Author, AppError> {
        self.repo.get(id).await
    }

    pub async fn create(
        &self,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> Result<Author, AppError> {
        let name = normalize_name(name)?;
        self.repo.create(&name, transcription, ndl_id).await
    }

    pub async fn update(
        &self,
        id: i64,
        name: &str,
        transcription: Option<&str>,
        ndl_id: Option<&str>,
    ) -> Result<(), AppError> {
        let name = normalize_name(name)?;
        self.repo.update(id, &name, transcription, ndl_id).await
    }

    pub async fn delete(&self, id: i64) -> Result<(), AppError> {
        self.repo.delete(id).await
    }
}

fn normalize_name(name: &str) -> Result<String, AppError> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::Validation("Name is required".to_string()));
    }
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::author::Author;

    #[derive(Default)]
    struct FakeRepository;

    impl AuthorRepository for FakeRepository {
        async fn list(&self) -> Result<Vec<Author>, AppError> {
            Ok(Vec::new())
        }

        async fn get(&self, _id: i64) -> Result<Author, AppError> {
            Err(AppError::NotFound)
        }

        async fn create(
            &self,
            name: &str,
            transcription: Option<&str>,
            ndl_id: Option<&str>,
        ) -> Result<Author, AppError> {
            Ok(Author {
                id: 1,
                name: name.to_string(),
                transcription: transcription.map(str::to_string),
                ndl_id: ndl_id.map(str::to_string),
            })
        }

        async fn update(
            &self,
            _id: i64,
            _name: &str,
            _transcription: Option<&str>,
            _ndl_id: Option<&str>,
        ) -> Result<(), AppError> {
            Ok(())
        }

        async fn delete(&self, _id: i64) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn create_normalizes_name() {
        let author = AuthorService::new(FakeRepository)
            .create("  Alice  ", Some("アリス"), Some("ndl-1"))
            .await
            .unwrap();
        assert_eq!(author.name, "Alice");
    }

    #[tokio::test]
    async fn empty_name_is_rejected() {
        let error = AuthorService::new(FakeRepository)
            .create("  ", None, None)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }
}
