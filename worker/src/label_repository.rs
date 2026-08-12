use dantalian::{
    application::error::AppError, domain::label::Label, ports::label_repository::LabelRepository,
};
use worker::{D1Database, D1Type};

pub struct D1LabelRepository {
    db: D1Database,
}

impl D1LabelRepository {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    fn map_error(error: worker::Error) -> AppError {
        AppError::Database(error.to_string())
    }

    fn bind_id(id: i64) -> Result<D1Type<'static>, AppError> {
        let id = i32::try_from(id)
            .map_err(|_| AppError::Validation("Label id is out of range".to_string()))?;
        Ok(D1Type::Integer(id))
    }
}

impl LabelRepository for D1LabelRepository {
    async fn list(&self) -> Result<Vec<Label>, AppError> {
        let result = self
            .db
            .prepare("SELECT id, name FROM labels ORDER BY name")
            .all()
            .await
            .map_err(Self::map_error)?;
        result.results::<Label>().map_err(Self::map_error)
    }

    async fn get_or_create(&self, name: &str) -> Result<Label, AppError> {
        let name = D1Type::Text(name);
        self.db
            .prepare("INSERT OR IGNORE INTO labels (name) VALUES (?)")
            .bind_refs(&name)
            .map_err(Self::map_error)?
            .run()
            .await
            .map_err(Self::map_error)?;
        self.db
            .prepare("SELECT id, name FROM labels WHERE name = ?")
            .bind_refs(&name)
            .map_err(Self::map_error)?
            .first::<Label>(None)
            .await
            .map_err(Self::map_error)?
            .ok_or_else(|| AppError::Database("label lookup returned no row".to_string()))
    }

    async fn rename(&self, id: i64, name: &str) -> Result<(), AppError> {
        let name = D1Type::Text(name);
        let id = Self::bind_id(id)?;
        let result = self
            .db
            .prepare("UPDATE labels SET name = ? WHERE id = ?")
            .bind_refs([&name, &id])
            .map_err(Self::map_error)?
            .run()
            .await
            .map_err(Self::map_error)?;
        let changed = result
            .meta()
            .map_err(Self::map_error)?
            .and_then(|meta| meta.changes)
            .unwrap_or_default();
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<(), AppError> {
        let id = Self::bind_id(id)?;
        let result = self
            .db
            .prepare("DELETE FROM labels WHERE id = ?")
            .bind_refs(&id)
            .map_err(Self::map_error)?
            .run()
            .await
            .map_err(Self::map_error)?;
        let changed = result
            .meta()
            .map_err(Self::map_error)?
            .and_then(|meta| meta.changes)
            .unwrap_or_default();
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }
}
