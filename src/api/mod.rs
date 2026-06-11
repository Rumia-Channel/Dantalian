pub mod books;
pub mod borrowers;
pub mod cds;
pub mod copies;
pub mod grand_series;
pub mod series;
pub mod settings;
pub mod tracks;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};

const COVER_MAX_BYTES: usize = 10 * 1024 * 1024;
const AUDIO_MAX_BYTES: usize = 100 * 1024 * 1024;
const EPUB_MAX_BYTES: usize = 500 * 1024 * 1024;

pub fn routes() -> axum::Router<crate::AppState> {
    axum::Router::new()
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
                .layer(DefaultBodyLimit::max(COVER_MAX_BYTES))
                .delete(books::delete_cover),
        )
        .route(
            "/books/{id}/epub",
            post(books::upload_epub)
                .layer(DefaultBodyLimit::max(EPUB_MAX_BYTES))
                .delete(books::delete_epub),
        )
        .route("/authors", get(books::list_authors))
        .route("/authors", post(books::create_author))
        .route("/authors/{id}", get(books::get_author))
        .route("/authors/{id}", put(books::update_author))
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
        .route(
            "/books/{id}/tracks/{track_id}/audio",
            post(tracks::upload_track_audio)
                .layer(DefaultBodyLimit::max(AUDIO_MAX_BYTES))
                .delete(tracks::delete_track_audio),
        )
        .route("/cds", get(cds::list_cds).post(cds::cd_register))
        .route("/track-metadata/search", get(cds::search_track_metadata))
        .route("/cds/{id}", put(cds::update_cd).delete(cds::delete_cd))
        .route(
            "/cds/{id}/cover",
            post(cds::upload_cd_cover)
                .layer(DefaultBodyLimit::max(COVER_MAX_BYTES))
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
                .layer(DefaultBodyLimit::max(AUDIO_MAX_BYTES))
                .delete(cds::delete_cd_track_audio),
        )
        .route(
            "/cds/{id}/authors/{author_id}",
            post(cds::add_cd_author)
                .delete(cds::remove_cd_author)
                .put(cds::update_cd_author_order),
        )
}
