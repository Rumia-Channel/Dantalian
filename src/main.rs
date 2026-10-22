#![cfg(feature = "native")]

mod epub;

use axum::{Router, response::Html};
use dantalian::db::Db;
use dantalian::{AppState, api, audio_encoding, backup, media_sync};
use reqwest::Client;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::EnvFilter;

const ASSET_VERSION: &str = env!("ASSET_VERSION");
const GENERATED_LICENSE_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/licenses.html"));

fn serve_html(path: &str) -> Html<String> {
    let html = std::fs::read_to_string(path).unwrap_or_else(|_| "Page not found".to_string());
    Html(html.replace("ASSET_VERSION", ASSET_VERSION))
}

async fn serve_index() -> Html<String> {
    serve_html("static/index.html")
}

async fn serve_books() -> Html<String> {
    serve_html("static/books/index.html")
}

async fn serve_reader() -> Html<String> {
    serve_html("static/reader/index.html")
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

async fn serve_music() -> Html<String> {
    serve_html("static/music/index.html")
}

async fn serve_licenses() -> Html<String> {
    Html(GENERATED_LICENSE_HTML.to_owned())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("dantalian=info")),
        )
        .init();

    let auth_config = dantalian::auth::AuthConfig::from_env();
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let bind_ip: IpAddr = bind_addr.parse().unwrap_or_else(|_| {
        eprintln!("BIND_ADDR が不正です: {bind_addr} (例: 127.0.0.1, 0.0.0.0)");
        std::process::exit(1);
    });
    if auth_config.required() {
        tracing::info!("Native API authentication is enabled (DANTALIAN_API_TOKEN)");
    } else if !bind_ip.is_loopback() {
        eprintln!(
            "BIND_ADDR={bind_ip} はネットワークに公開されます。認証なしでは起動しません。\
             DANTALIAN_API_TOKEN を設定するか、BIND_ADDR=127.0.0.1 で起動してください。"
        );
        std::process::exit(1);
    } else {
        tracing::info!(%bind_ip, "Native API authentication is disabled (loopback bind only)");
    }

    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| {
        dirs::document_dir()
            .or_else(dirs::data_dir)
            .expect("Could not determine data directory")
            .join("Dantalian")
            .to_string_lossy()
            .to_string()
    });
    let db_dir = format!("{}{}db", data_dir, std::path::MAIN_SEPARATOR);
    let images_dir = format!("{}{}images", data_dir, std::path::MAIN_SEPARATOR);
    let audio_dir = format!("{}{}audio", data_dir, std::path::MAIN_SEPARATOR);
    let epubs_dir = format!("{}{}epubs", data_dir, std::path::MAIN_SEPARATOR);
    let uploads_dir = format!("{}{}uploads", data_dir, std::path::MAIN_SEPARATOR);
    let object_storage = Arc::new(
        dantalian::adapters::native_object_storage::NativeObjectStorage::new(&data_dir, ""),
    );
    std::fs::create_dir_all(&db_dir).expect("Failed to create db directory");
    std::fs::create_dir_all(&images_dir).expect("Failed to create images directory");
    std::fs::create_dir_all(&audio_dir).expect("Failed to create audio directory");
    std::fs::create_dir_all(&epubs_dir).expect("Failed to create epubs directory");
    std::fs::create_dir_all(&uploads_dir).expect("Failed to create upload directory");
    api::cleanup_stale_uploads(&uploads_dir);

    let db_path = format!("{}{}dantalian.db", db_dir, std::path::MAIN_SEPARATOR);
    tracing::info!(%data_dir, %db_path, %images_dir, %audio_dir, %epubs_dir, "Data directories");

    let db = Db::new(&db_path).expect("Failed to initialize database");

    let backup_config = backup::BackupConfig::load(&db);
    if backup_config.enabled {
        tracing::info!(
            "Backup enabled (retention: {} files)",
            backup_config.retention
        );
        backup::start_scheduled_backup(db.clone());
    }

    let media_sync_config = media_sync::MediaSyncConfig::load(
        &db,
        images_dir.clone(),
        audio_dir.clone(),
        epubs_dir.clone(),
    );
    if media_sync_config.enabled {
        tracing::info!(
            "Media sync enabled (types: {})",
            media_sync_config
                .types
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    // Always start the worker; it idles when media_sync.enabled is false and
    // re-reads configuration on every iteration.
    media_sync::start_scheduled_media_sync(
        db.clone(),
        images_dir.clone(),
        audio_dir.clone(),
        epubs_dir.clone(),
    );

    let client = Client::builder()
        .cookie_store(true)
        .connect_timeout(Duration::from_secs(15))
        .build()
        .expect("Failed to build HTTP client");

    let client_ipv4 = Client::builder()
        .cookie_store(true)
        .connect_timeout(Duration::from_secs(15))
        .local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
        .build()
        .expect("Failed to build IPv4 HTTP client");

    let discogs_token = db
        .get_setting("discogs_token")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::var("DISCOGS_TOKEN").unwrap_or_default());

    let musicbrainz_contact = db
        .get_setting("musicbrainz_contact")
        .filter(|s| !s.trim().is_empty() && !s.contains("example.com"))
        .or_else(|| std::env::var("MUSICBRAINZ_CONTACT").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.contains("example.com"))
        .unwrap_or_else(|| "+https://github.com/Rumia-Channel/dantalian".to_string());

    let images_dir_arc = Arc::new(images_dir);
    let audio_dir_arc = Arc::new(audio_dir);
    let audio_encoding_notify = Arc::new(tokio::sync::Notify::new());
    let epubs_dir_arc = Arc::new(epubs_dir);
    let uploads_dir_arc = Arc::new(uploads_dir);

    let state = AppState {
        db,
        client,
        client_ipv4,
        object_storage,
        images_dir: images_dir_arc.clone(),
        audio_dir: audio_dir_arc.clone(),
        audio_encoding_notify: audio_encoding_notify.clone(),
        epubs_dir: epubs_dir_arc.clone(),
        uploads_dir: uploads_dir_arc.clone(),
        discogs_token,
        musicbrainz_contact,
        auth: auth_config.clone(),
    };

    let _audio_encoding_worker = audio_encoding::start_background_worker(
        state.db.clone(),
        state.audio_dir.as_ref().clone(),
        state.audio_encoding_notify.clone(),
    );

    // 中断したチャンクアップロードを定期的に破棄する（起動時にも一度実行済み）。
    let cleanup_uploads_dir = uploads_dir_arc.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            let directory = cleanup_uploads_dir.as_ref().clone();
            if let Err(error) = tokio::task::spawn_blocking(move || {
                dantalian::api::cleanup_stale_uploads(&directory);
            })
            .await
            {
                tracing::warn!("stale upload cleanup failed: {error}");
            }
        }
    });

    let shutdown_db = state.db.clone();

    let app = Router::new()
        .route("/", axum::routing::get(serve_index))
        .route("/books", axum::routing::get(serve_books))
        .route("/books/", axum::routing::get(serve_books))
        .route("/reader", axum::routing::get(serve_reader))
        .route("/reader/", axum::routing::get(serve_reader))
        .route("/register", axum::routing::get(serve_register))
        .route("/register/", axum::routing::get(serve_register))
        .route("/manage", axum::routing::get(serve_manage))
        .route("/manage/", axum::routing::get(serve_manage))
        .route("/licenses", axum::routing::get(serve_licenses))
        .route("/licenses/", axum::routing::get(serve_licenses))
        .route_service("/LICENSE", ServeFile::new("LICENSE"))
        .route_service("/NOTICE", ServeFile::new("NOTICE"))
        .route("/edit", axum::routing::get(serve_edit))
        .route("/edit/", axum::routing::get(serve_edit))
        .route("/authors", axum::routing::get(serve_authors))
        .route("/authors/", axum::routing::get(serve_authors))
        .route("/music", axum::routing::get(serve_music))
        .route("/music/", axum::routing::get(serve_music))
        .nest("/api", api::routes())
        .nest_service("/images", ServeDir::new(images_dir_arc.as_ref()))
        .nest_service("/audio", ServeDir::new(audio_dir_arc.as_ref()))
        .nest("/epubs", epub::routes(epubs_dir_arc).with_state(()))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(axum::middleware::from_fn_with_state(
            auth_config.clone(),
            dantalian::auth::require_auth,
        ))
        .layer(axum::middleware::from_fn(
            dantalian::http_security::security_headers,
        ))
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let listener = tokio::net::TcpListener::bind((bind_ip, port))
        .await
        .unwrap();
    eprintln!("Server running on http://{}:{}", bind_ip, port);
    tracing::info!("Server running on http://{}:{}", bind_ip, port);
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
