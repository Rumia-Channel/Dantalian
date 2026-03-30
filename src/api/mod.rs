pub mod books;
pub mod grand_series;
pub mod series;

use axum::routing::{delete, get, post, put};

pub fn routes() -> axum::Router<crate::AppState> {
    axum::Router::new()
        .route("/books", post(books::register))
        .route("/books", get(books::list))
        .route("/books/{id}", delete(books::delete))
        .route("/books/{id}", put(books::update_book))
        .route("/books/{id}/series", put(books::set_series))
        .route("/books/{id}/authors/{author_id}", post(books::add_book_author).delete(books::remove_book_author).put(books::update_book_author_order))
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
        .route("/grand-series/{id}/items/{item_type}/{item_id}", delete(grand_series::remove_item))
}
