pub mod books;

use axum::routing::{delete, get, post};

pub fn routes() -> axum::Router<crate::AppState> {
    axum::Router::new()
        .route("/books", post(books::register))
        .route("/books", get(books::list))
        .route("/books/{id}", delete(books::delete))
}
