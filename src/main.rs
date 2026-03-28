mod api;
mod db;
mod external;

use axum::{Router, response::Html};
use db::Db;
use reqwest::Client;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

const ASSET_VERSION: &str = env!("ASSET_VERSION");

async fn serve_index() -> Html<String> {
    let html = std::fs::read_to_string("static/index.html")
        .unwrap_or_else(|_| "index.html not found".to_string());
    Html(html.replace("ASSET_VERSION", ASSET_VERSION))
}

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub client: Client,
    pub images_dir: Arc<String>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("tsukuyomi=info")),
        )
        .init();

    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| {
        dirs::document_dir()
            .or_else(dirs::data_dir)
            .expect("Could not determine data directory")
            .join("Tsukuyomi")
            .to_string_lossy()
            .to_string()
    });
    let db_dir = format!("{}{}db", data_dir, std::path::MAIN_SEPARATOR);
    let images_dir = format!("{}{}images", data_dir, std::path::MAIN_SEPARATOR);
    std::fs::create_dir_all(&db_dir).expect("Failed to create db directory");
    std::fs::create_dir_all(&images_dir).expect("Failed to create images directory");

    let db_path = format!("{}{}books.db", db_dir, std::path::MAIN_SEPARATOR);
    tracing::info!(%data_dir, %db_path, %images_dir, "Data directories");

    let db = Db::new(&db_path).expect("Failed to initialize database");
    let client = Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to build HTTP client");

    let state = AppState {
        db,
        client,
        images_dir: Arc::new(images_dir.clone()),
    };

    let images_dir_arc = Arc::new(images_dir);
    let app = Router::new()
        .route("/", axum::routing::get(serve_index))
        .nest("/api", api::routes())
        .nest_service("/images", ServeDir::new(images_dir_arc.as_ref()))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    eprintln!("Server running on http://localhost:{}", port);
    tracing::info!("Server running on http://localhost:{}", port);
    axum::serve(listener, app).await.unwrap();
}
