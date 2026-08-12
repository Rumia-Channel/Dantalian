use dantalian::{
    application::author::AuthorService,
    domain::author::{CreateAuthor, UpdateAuthor},
};
use worker::{Request, Response, Result, RouteContext};

use crate::{
    author_repository::D1AuthorRepository,
    error::{error_response, parse_id, parse_json},
};

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let service = AuthorService::new(D1AuthorRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |authors| Response::from_json(&authors))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CreateAuthor>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let service = AuthorService::new(D1AuthorRepository::new(ctx.d1("DB")?));
    service
        .create(
            &request.name,
            request.transcription.as_deref(),
            request.ndl_id.as_deref(),
        )
        .await
        .map_or_else(error_response, |author| {
            Response::from_json(&author).map(|response| response.with_status(201))
        })
}

pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let service = AuthorService::new(D1AuthorRepository::new(ctx.d1("DB")?));
    service
        .get(id)
        .await
        .map_or_else(error_response, |author| Response::from_json(&author))
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<UpdateAuthor>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let service = AuthorService::new(D1AuthorRepository::new(ctx.d1("DB")?));
    service
        .update(
            id,
            &request.name,
            request.transcription.as_deref(),
            request.ndl_id.as_deref(),
        )
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
    let service = AuthorService::new(D1AuthorRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
