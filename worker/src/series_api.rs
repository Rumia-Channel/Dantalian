use dantalian::{
    application::series::SeriesService,
    domain::series::{CreateSeries, RenameSeries},
};
use worker::{Request, Response, Result, RouteContext};

use crate::{
    error::{error_response, parse_id, parse_json},
    series_repository::D1SeriesRepository,
};

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let service = SeriesService::new(D1SeriesRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |series| Response::from_json(&series))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CreateSeries>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let service = SeriesService::new(D1SeriesRepository::new(ctx.d1("DB")?));
    service
        .create(&request.name)
        .await
        .map_or_else(error_response, |series| {
            Response::from_json(&series).map(|response| response.with_status(201))
        })
}

pub async fn rename(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<RenameSeries>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let service = SeriesService::new(D1SeriesRepository::new(ctx.d1("DB")?));
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
    let service = SeriesService::new(D1SeriesRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
