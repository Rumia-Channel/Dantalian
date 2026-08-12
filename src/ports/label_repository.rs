use std::future::Future;

use crate::{application::error::AppError, domain::label::Label};

pub trait LabelRepository {
    fn list(&self) -> impl Future<Output = Result<Vec<Label>, AppError>>;

    fn get_or_create(&self, name: &str) -> impl Future<Output = Result<Label, AppError>>;

    fn rename(&self, id: i64, name: &str) -> impl Future<Output = Result<(), AppError>>;

    fn delete(&self, id: i64) -> impl Future<Output = Result<(), AppError>>;
}
