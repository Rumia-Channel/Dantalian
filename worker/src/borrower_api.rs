use dantalian::{
    application::borrower::BorrowerService,
    domain::borrower::{CreateBorrower, UpdateBorrower},
};
use worker::{Request, Response, Result, RouteContext};

use crate::{
    borrower_repository::D1BorrowerRepository,
    error::{error_response, parse_id, parse_json},
};

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let service = BorrowerService::new(D1BorrowerRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |borrowers| Response::from_json(&borrowers))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CreateBorrower>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let service = BorrowerService::new(D1BorrowerRepository::new(ctx.d1("DB")?));
    service
        .create(&request.name, request.notes.as_deref())
        .await
        .map_or_else(error_response, |borrower| {
            Response::from_json(&borrower).map(|response| response.with_status(201))
        })
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<UpdateBorrower>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let service = BorrowerService::new(D1BorrowerRepository::new(ctx.d1("DB")?));
    service
        .update(id, request.name.as_deref(), request.notes.as_deref())
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
    let service = BorrowerService::new(D1BorrowerRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
