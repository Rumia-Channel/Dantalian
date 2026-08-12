mod borrower_api;
mod borrower_repository;
mod label_api;
mod label_repository;
mod series_api;
mod series_repository;
mod storage_location_api;
mod storage_location_repository;

use worker::*;

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get_async("/api/health", |_req, _ctx| async move {
            Response::from_json(&serde_json::json!({
                "ok": true,
                "runtime": "cloudflare-worker",
            }))
        })
        .get_async("/api/series", series_api::list)
        .post_async("/api/series", series_api::create)
        .put_async("/api/series/:id", series_api::rename)
        .delete_async("/api/series/:id", series_api::delete)
        .get_async("/api/labels", label_api::list)
        .post_async("/api/labels", label_api::create)
        .put_async("/api/labels/:id", label_api::rename)
        .get_async("/api/borrowers", borrower_api::list)
        .post_async("/api/borrowers", borrower_api::create)
        .put_async("/api/borrowers/:id", borrower_api::update)
        .delete_async("/api/borrowers/:id", borrower_api::delete)
        .delete_async("/api/labels/:id", label_api::delete)
        .get_async("/api/storage-locations", storage_location_api::list)
        .post_async("/api/storage-locations", storage_location_api::create)
        .put_async("/api/storage-locations/:id", storage_location_api::update)
        .delete_async("/api/storage-locations/:id", storage_location_api::delete)
        .run(req, env)
        .await
}
