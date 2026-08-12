use dantalian::{
    application::{error::AppError, storage_location::StorageLocationService},
    domain::storage_location::{CreateStorageLocation, UpdateStorageLocation},
};
use worker::{Request, Response, Result, RouteContext};

use crate::storage_location_repository::D1StorageLocationRepository;

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
    let service = StorageLocationService::new(D1StorageLocationRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |locations| Response::from_json(&locations))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request: CreateStorageLocation = req.json().await?;
    let service = StorageLocationService::new(D1StorageLocationRepository::new(ctx.d1("DB")?));
    service
        .create(&request.name, request.parent_id)
        .await
        .map_or_else(error_response, |location| {
            Response::from_json(&location).map(|response| response.with_status(201))
        })
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request: UpdateStorageLocation = req.json().await?;
    let id = ctx
        .param("id")
        .ok_or_else(|| worker::Error::RustError("missing storage location id".to_string()))?
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid storage location id".to_string()))?;
    let service = StorageLocationService::new(D1StorageLocationRepository::new(ctx.d1("DB")?));
    service
        .update(id, request.name.as_deref(), request.parent_id)
        .await
        .map_or_else(error_response, |_| {
            Response::empty().map(|response| response.with_status(204))
        })
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| worker::Error::RustError("missing storage location id".to_string()))?
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid storage location id".to_string()))?;
    let service = StorageLocationService::new(D1StorageLocationRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
