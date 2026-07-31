pub mod audio;
pub mod books;
pub mod borrowers;
pub mod cds;
pub mod copies;
pub mod grand_series;
pub mod labels;
pub mod media_sync;
pub mod playlists;
pub mod series;
pub mod settings;
pub mod storage_locations;
pub mod tracks;
pub(crate) mod upload_chunks;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};

pub(crate) const COVER_MAX_BYTES: usize = 10 * 1024 * 1024;
pub(crate) const AUDIO_MAX_BYTES: usize = 100 * 1024 * 1024;
pub(crate) const EPUB_MAX_BYTES: usize = 500 * 1024 * 1024;

/// ルートに掛けるボディ上限の天井値。実効上限は設定(upload.*_max_mb)で決め、
/// ハンドラ側で検査する。ルート側はこの天井まで読み取りを許可するだけ。
pub(crate) const UPLOAD_ROUTE_LIMIT_BYTES: usize = 4 * 1024 * 1024 * 1024;

pub(crate) const KEY_UPLOAD_COVER_MB: &str = "upload.cover_max_mb";
pub(crate) const KEY_UPLOAD_AUDIO_MB: &str = "upload.audio_max_mb";
pub(crate) const KEY_UPLOAD_FILE_MB: &str = "upload.file_max_mb";

/// 設定からアップロード上限(MB指定)を読み、バイト数に変換して返す。
/// 未設定/不正値なら default_bytes、天井(UPLOAD_ROUTE_LIMIT_BYTES)でキャップ。
pub(crate) fn upload_limit_bytes(
    state: &crate::AppState,
    key: &str,
    default_bytes: usize,
) -> usize {
    state
        .db
        .get_setting(key)
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|mb| (mb as usize).saturating_mul(1024 * 1024))
        .filter(|&b| b > 0)
        .map(|b| b.min(UPLOAD_ROUTE_LIMIT_BYTES))
        .unwrap_or(default_bytes)
}

pub fn routes() -> axum::Router<crate::AppState> {
    axum::Router::new()
        .route("/audio/stream/{file_hash}", get(audio::stream))
        .route("/books", post(books::register))
        .route("/books/isdn", post(books::isdn_register))
        .route("/books/manual", post(books::manual_register))
        .route("/books", get(books::list))
        .route("/books/{id}", delete(books::delete))
        .route("/books/{id}", put(books::update_book))
        .route("/books/{id}/series", put(books::set_series))
        .route(
            "/books/{id}/authors/{author_id}",
            post(books::add_book_author)
                .delete(books::remove_book_author)
                .put(books::update_book_author_order),
        )
        .route(
            "/books/{id}/cover",
            post(books::upload_cover)
                .layer(DefaultBodyLimit::max(UPLOAD_ROUTE_LIMIT_BYTES))
                .delete(books::delete_cover),
        )
        .route(
            "/books/{id}/epub",
            post(books::upload_epub)
                .layer(DefaultBodyLimit::max(UPLOAD_ROUTE_LIMIT_BYTES))
                .delete(books::delete_epub),
        )
        .route("/authors", get(books::list_authors))
        .route("/authors", post(books::create_author))
        .route(
            "/authors/{id}",
            get(books::get_author)
                .put(books::update_author)
                .delete(books::delete_author),
        )
        .route("/series", post(series::create))
        .route("/series", get(series::list))
        .route("/series/{id}", put(series::rename))
        .route("/series/{id}", delete(series::delete))
        .route("/grand-series", post(grand_series::create))
        .route("/grand-series", get(grand_series::list))
        .route("/grand-series/{id}", put(grand_series::rename))
        .route("/grand-series/{id}", delete(grand_series::delete))
        .route("/grand-series/{id}/items", post(grand_series::add_item))
        .route(
            "/grand-series/{id}/items/{item_type}/{item_id}",
            delete(grand_series::remove_item),
        )
        .route("/storage-locations", post(storage_locations::create))
        .route("/storage-locations", get(storage_locations::list))
        .route("/storage-locations/{id}", put(storage_locations::update))
        .route("/storage-locations/{id}", delete(storage_locations::delete))
        .route("/labels", post(labels::create))
        .route("/labels", get(labels::list))
        .route("/labels/{id}", put(labels::update))
        .route("/labels/{id}", delete(labels::delete))
        .route("/playlists", get(playlists::list).post(playlists::create))
        .route(
            "/playlists/{id}",
            get(playlists::get)
                .put(playlists::update)
                .delete(playlists::delete),
        )
        .route(
            "/playlists/{id}/tracks",
            put(playlists::set_tracks).post(playlists::add_track),
        )
        .route(
            "/playlists/{id}/tracks/{track_id}",
            delete(playlists::remove_track),
        )
        .route("/books/{id}/copies", get(copies::list_copies))
        .route("/books/{id}/copies", post(copies::create_copy))
        .route("/copies/{id}", put(copies::update_copy))
        .route("/copies/{id}", delete(copies::delete_copy))
        .route("/copies/{id}/lend", post(copies::lend_copy))
        .route("/copies/{id}/return", post(copies::return_copy))
        .route("/copies/{id}/history", get(copies::get_lending_history))
        .route("/borrowers", get(borrowers::list))
        .route("/borrowers", post(borrowers::create))
        .route("/borrowers/{id}", put(borrowers::update))
        .route("/borrowers/{id}", delete(borrowers::delete))
        .route(
            "/settings",
            get(settings::get_settings).put(settings::update_settings),
        )
        .route("/books/{id}/tracks", get(tracks::list_tracks))
        .route("/books/{id}/tracks/{track_id}", put(tracks::update_track))
        .route(
            "/books/{id}/tracks/{track_id}/metadata",
            get(tracks::get_book_track_metadata).put(tracks::put_book_track_metadata),
        )
        .route(
            "/cds/{id}/tracks/{tid}/metadata",
            get(cds::get_cd_track_metadata).put(cds::put_cd_track_metadata),
        )
        .route(
            "/cds/{id}/metadata",
            get(cds::get_cd_metadata).put(cds::put_cd_metadata),
        )
        .route("/cds/{id}/album-tags", get(cds::get_cd_album_tags))
        .route(
            "/books/{id}/tracks/{track_id}/audio",
            post(tracks::upload_track_audio)
                .layer(DefaultBodyLimit::max(UPLOAD_ROUTE_LIMIT_BYTES))
                .delete(tracks::delete_track_audio),
        )
        .route("/cds", get(cds::list_cds).post(cds::cd_register))
        .route("/track-metadata/search", get(cds::search_track_metadata))
        .route("/cds/{id}", put(cds::update_cd).delete(cds::delete_cd))
        .route(
            "/cds/{id}/cover",
            post(cds::upload_cd_cover)
                .layer(DefaultBodyLimit::max(UPLOAD_ROUTE_LIMIT_BYTES))
                .delete(cds::delete_cd_cover),
        )
        .route(
            "/cds/{id}/tracks",
            get(cds::list_cd_tracks).post(cds::add_cd_track),
        )
        .route(
            "/cds/{id}/tracks/{tid}",
            put(cds::update_cd_track).delete(cds::delete_cd_track),
        )
        .route(
            "/cds/{id}/tracks/{tid}/audio",
            post(cds::upload_cd_track_audio)
                .layer(DefaultBodyLimit::max(UPLOAD_ROUTE_LIMIT_BYTES))
                .delete(cds::delete_cd_track_audio),
        )
        .route(
            "/cds/{id}/authors/from-names",
            post(cds::add_cd_authors_from_names),
        )
        .route(
            "/cds/{id}/authors/{author_id}",
            post(cds::add_cd_author)
                .delete(cds::remove_cd_author)
                .put(cds::update_cd_author_order),
        )
        .route("/media-sync/run", post(media_sync::run_media_sync))
}
