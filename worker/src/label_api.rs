use dantalian::{
    application::{error::AppError, label::LabelService},
    domain::label::{CreateLabel, RenameLabel},
};
use worker::{Request, Response, Result, RouteContext};

use crate::label_repository::D1LabelRepository;

fn error_response(error: AppError) -> Result<Response> {
    let status = match error {
        AppError::Validation(_) => 400,
        AppError::NotFound => 404,
        AppError::Conflict(_) => 409,
        AppError::Database(_) | AppError::Internal(_) => 500,
    };
    Response::from_json(&serde_json::json!({ "error": error.to_string() }))
        .map(|response| response.with_status(status))
}

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let service = LabelService::new(D1LabelRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |labels| Response::from_json(&labels))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request: CreateLabel = req.json().await?;
    let service = LabelService::new(D1LabelRepository::new(ctx.d1("DB")?));
    service
        .create(&request.name)
        .await
        .map_or_else(error_response, |label| {
            Response::from_json(&label).map(|response| response.with_status(201))
        })
}

pub async fn rename(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request: RenameLabel = req.json().await?;
    let id = ctx
        .param("id")
        .ok_or_else(|| worker::Error::RustError("missing label id".to_string()))?
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid label id".to_string()))?;
    let service = LabelService::new(D1LabelRepository::new(ctx.d1("DB")?));
    service
        .rename(id, &request.name)
        .await
        .map_or_else(error_response, |_| {
            Response::empty().map(|response| response.with_status(204))
        })
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| worker::Error::RustError("missing label id".to_string()))?
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid label id".to_string()))?;
    let service = LabelService::new(D1LabelRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
