pub mod books;
pub mod series;

use axum::routing::{delete, get, post, put};

pub fn routes() -> axum::Router<crate::AppState> {
    axum::Router::new()
        .route("/books", post(books::register))
        .route("/books", get(books::list))
        .route("/books/{id}", delete(books::delete))
        .route("/books/{id}/series", put(books::set_series))
        .route("/authors/{id}", get(books::get_author))
        .route("/authors", get(books::search_author))
        .route("/series", post(series::create))
        .route("/series", get(series::list))
        .route("/series/{id}", put(series::rename))
        .route("/series/{id}", delete(series::delete))
}
