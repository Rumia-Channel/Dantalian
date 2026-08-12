use dantalian::{
    application::label::LabelService,
    domain::label::{CreateLabel, RenameLabel},
};
use worker::{Request, Response, Result, RouteContext};

use crate::{
    error::{error_response, parse_id, parse_json},
    label_repository::D1LabelRepository,
};

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let service = LabelService::new(D1LabelRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |labels| Response::from_json(&labels))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CreateLabel>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let service = LabelService::new(D1LabelRepository::new(ctx.d1("DB")?));
    service
        .create(&request.name)
        .await
        .map_or_else(error_response, |label| {
            Response::from_json(&label).map(|response| response.with_status(201))
        })
}

pub async fn rename(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<RenameLabel>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let service = LabelService::new(D1LabelRepository::new(ctx.d1("DB")?));
    service
        .rename(id, &request.name)
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
    let service = LabelService::new(D1LabelRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
