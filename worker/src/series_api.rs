use dantalian::{
    application::{error::AppError, series::SeriesService},
    domain::series::{CreateSeries, RenameSeries},
};
use serde::Serialize;
use worker::{Request, Response, Result, RouteContext};

use crate::series_repository::D1SeriesRepository;

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn error_response(error: AppError) -> Result<Response> {
    let status = match error {
        AppError::Validation(_) => 400,
        AppError::NotFound => 404,
        AppError::Conflict(_) => 409,
        AppError::Database(_) | AppError::Internal(_) => 500,
    };
    Response::from_json(&ErrorResponse {
        error: error.to_string(),
    })
    .map(|response| response.with_status(status))
}

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let service = SeriesService::new(D1SeriesRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |series| Response::from_json(&series))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request: CreateSeries = req.json().await?;
    let service = SeriesService::new(D1SeriesRepository::new(ctx.d1("DB")?));
    service
        .create(&request.name)
        .await
        .map_or_else(error_response, |series| {
            Response::from_json(&series).map(|response| response.with_status(201))
        })
}

pub async fn rename(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request: RenameSeries = req.json().await?;
    let id = ctx
        .param("id")
        .ok_or_else(|| worker::Error::RustError("missing series id".to_string()))?
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid series id".to_string()))?;
    let service = SeriesService::new(D1SeriesRepository::new(ctx.d1("DB")?));
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
        .ok_or_else(|| worker::Error::RustError("missing series id".to_string()))?
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid series id".to_string()))?;
    let service = SeriesService::new(D1SeriesRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
