mod api;
mod db;
mod external;

use axum::Router;
use db::Db;
use reqwest::Client;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub client: Client,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("tsukuyomi=info")),
        )
        .init();

    let db = Db::new().expect("Failed to initialize database");
    let client = Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to build HTTP client");

    let state = AppState { db, client };

    let app = Router::new()
        .nest("/api", api::routes())
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    println!("Server running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
