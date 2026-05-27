pub mod books;
pub mod borrowers;
pub mod copies;
pub mod grand_series;
pub mod series;
pub mod settings;

use axum::routing::{delete, get, post, put};

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
            post(books::upload_cover).delete(books::delete_cover),
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
        .route("/settings", get(settings::get_settings).put(settings::update_settings))
}
