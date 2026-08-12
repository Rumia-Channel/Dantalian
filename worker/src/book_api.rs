use dantalian::application::book::BookService;
use worker::{Request, Response, Result, RouteContext};

use crate::{
    book_repository::D1BookRepository,
    error::{error_response, parse_id},
};

pub async fn list(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let service = BookService::new(D1BookRepository::new(ctx.d1("DB")?));
    service
        .list()
        .await
        .map_or_else(error_response, |books| Response::from_json(&books))
}

pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let id = match parse_id(&ctx, "id") {
        Ok(id) => id,
        Err(response) => return Ok(response),
    };
    let service = BookService::new(D1BookRepository::new(ctx.d1("DB")?));
    service
        .get(id)
        .await
        .map_or_else(error_response, |book| Response::from_json(&book))
}
