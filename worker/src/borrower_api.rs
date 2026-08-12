use dantalian::{
    application::{borrower::BorrowerService, error::AppError},
    domain::borrower::{CreateBorrower, UpdateBorrower},
};
use worker::{Request, Response, Result, RouteContext};

use crate::borrower_repository::D1BorrowerRepository;

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
    let service = BorrowerService::new(D1BorrowerRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |borrowers| Response::from_json(&borrowers))
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request: CreateBorrower = req.json().await?;
    let service = BorrowerService::new(D1BorrowerRepository::new(ctx.d1("DB")?));
    service
        .create(&request.name, request.notes.as_deref())
        .await
        .map_or_else(error_response, |borrower| {
            Response::from_json(&borrower).map(|response| response.with_status(201))
        })
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request: UpdateBorrower = req.json().await?;
    let id = ctx
        .param("id")
        .ok_or_else(|| worker::Error::RustError("missing borrower id".to_string()))?
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid borrower id".to_string()))?;
    let service = BorrowerService::new(D1BorrowerRepository::new(ctx.d1("DB")?));
    service
        .update(id, request.name.as_deref(), request.notes.as_deref())
        .await
        .map_or_else(error_response, |_| {
            Response::empty().map(|response| response.with_status(204))
        })
}

pub async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| worker::Error::RustError("missing borrower id".to_string()))?
        .parse::<i64>()
        .map_err(|_| worker::Error::RustError("invalid borrower id".to_string()))?;
    let service = BorrowerService::new(D1BorrowerRepository::new(ctx.d1("DB")?));
    service.delete(id).await.map_or_else(error_response, |_| {
        Response::empty().map(|response| response.with_status(204))
    })
}
