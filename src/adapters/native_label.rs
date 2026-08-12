use crate::{
    application::error::AppError, db::Db, domain::label::Label,
    ports::label_repository::LabelRepository,
};

#[derive(Clone)]
pub struct NativeLabelRepository {
    db: Db,
}

impl NativeLabelRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl LabelRepository for NativeLabelRepository {
    async fn list(&self) -> Result<Vec<Label>, AppError> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            db.list_labels()
                .map(|labels| labels.into_iter().map(Into::into).collect())
                .map_err(|error| AppError::Database(error.to_string()))
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn get_or_create(&self, name: &str) -> Result<Label, AppError> {
        let db = self.db.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            db.get_or_create_label(&name)
                .map(Into::into)
                .map_err(|error| AppError::Database(error.to_string()))
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?
    }

    async fn rename(&self, id: i64, name: &str) -> Result<(), AppError> {
        let updated = self
            .db
            .rename_label(id, name)
            .map_err(|error| AppError::Database(error.to_string()))?;
        if updated {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        let deleted = self
            .db
            .delete_label(id)
            .map_err(|error| AppError::Database(error.to_string()))?;
        if deleted {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
}

impl From<crate::db::Label> for Label {
    fn from(label: crate::db::Label) -> Self {
        Self {
            id: label.id,
            name: label.name,
        }
    }
}
