use dantalian::{
    application::storage_location::StorageLocationService,
    domain::storage_location::{CreateStorageLocation, UpdateStorageLocation},
};
use worker::{Request, Response, Result, RouteContext};

use crate::{
    error::{error_response, parse_id, parse_json},
    storage_location_repository::D1StorageLocationRepository,
};

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let service = StorageLocationService::new(D1StorageLocationRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |locations| Response::from_json(&locations))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CreateStorageLocation>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let service = StorageLocationService::new(D1StorageLocationRepository::new(ctx.d1("DB")?));
    service
        .create(&request.name, request.parent_id)
        .await
        .map_or_else(error_response, |location| {
            Response::from_json(&location).map(|response| response.with_status(201))
        })
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<UpdateStorageLocation>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let service = StorageLocationService::new(D1StorageLocationRepository::new(ctx.d1("DB")?));
    service
        .update(id, request.name.as_deref(), request.parent_id)
        .await
        .map_or_else(error_response, |_| {
            Response::empty().map(|response| response.with_status(204))
        })
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let service = StorageLocationService::new(D1StorageLocationRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
