mod api;
mod backup;
mod db;
mod db_models;
mod external;

use axum::{Router, response::Html};
use db::Db;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

const ASSET_VERSION: &str = env!("ASSET_VERSION");

fn serve_html(path: &str) -> Html<String> {
    let html = std::fs::read_to_string(path).unwrap_or_else(|_| "Page not found".to_string());
    Html(html.replace("ASSET_VERSION", ASSET_VERSION))
}

async fn serve_index() -> Html<String> {
    serve_html("static/index.html")
}

async fn serve_register() -> Html<String> {
    serve_html("static/register/index.html")
}

async fn serve_manage() -> Html<String> {
    serve_html("static/manage/index.html")
}

async fn serve_edit() -> Html<String> {
    serve_html("static/edit/index.html")
}

async fn serve_authors() -> Html<String> {
    serve_html("static/authors/index.html")
}

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub client: Client,
    pub client_ipv4: Client,
    pub images_dir: Arc<String>,
    pub discogs_token: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("tsukuyomi=info")),
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

    let db_path = format!("{}{}tsukuyomi.db", db_dir, std::path::MAIN_SEPARATOR);
    tracing::info!(%data_dir, %db_path, %images_dir, "Data directories");

    let db = Db::new(&db_path).expect("Failed to initialize database");

    let backup_config = backup::BackupConfig::load(&db);
    if backup_config.enabled {
        tracing::info!("Backup enabled (retention: {} files)", backup_config.retention);
        backup::start_scheduled_backup(db.clone());
    }

    let client = Client::builder()
        .cookie_store(true)
        .connect_timeout(Duration::from_secs(15))
        .build()
        .expect("Failed to build HTTP client");

    let client_ipv4 = client.clone();

    let discogs_token = std::env::var("DISCOGS_TOKEN").unwrap_or_default();

    let state = AppState {
        db,
        client,
        client_ipv4,
        images_dir: Arc::new(images_dir.clone()),
        discogs_token,
    };

    let shutdown_db = state.db.clone();

    let images_dir_arc = Arc::new(images_dir);
    let audio_dir = format!("{}{}audio", data_dir, std::path::MAIN_SEPARATOR);
    std::fs::create_dir_all(&audio_dir).expect("Failed to create audio directory");
    let audio_dir_arc = Arc::new(audio_dir);
    let app = Router::new()
        .route("/", axum::routing::get(serve_index))
        .route("/register", axum::routing::get(serve_register))
        .route("/register/", axum::routing::get(serve_register))
        .route("/manage", axum::routing::get(serve_manage))
        .route("/manage/", axum::routing::get(serve_manage))
        .route("/edit", axum::routing::get(serve_edit))
        .route("/edit/", axum::routing::get(serve_edit))
        .route("/authors", axum::routing::get(serve_authors))
        .route("/authors/", axum::routing::get(serve_authors))
        .nest("/api", api::routes())
        .nest_service("/images", ServeDir::new(images_dir_arc.as_ref()))
        .nest_service("/audio", ServeDir::new(audio_dir_arc.as_ref()))
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
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("Shutting down...");
            let config = backup::BackupConfig::load(&shutdown_db);
            if config.enabled {
                backup::perform_backup(&shutdown_db, &config).await;
            }
            tracing::info!("Goodbye.");
        })
        .await
        .unwrap();
}
