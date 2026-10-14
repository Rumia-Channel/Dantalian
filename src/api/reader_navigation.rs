use crate::{
    AppState,
    application::reader_navigation::{self, NavigationError, NavigationQuery},
};
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

fn error(error: NavigationError) -> Response {
    (
        StatusCode::from_u16(error.status()).expect("navigation error status is valid"),
        Json(error.body()),
    )
        .into_response()
}

pub async fn get(
    State(state): State<AppState>,
    query: Result<Query<NavigationQuery>, QueryRejection>,
) -> Response {
    let result = match query {
        Ok(Query(query)) => match query.validate() {
            Ok(request) => match state
                .db
                .reader_navigation(request.id, &request.content_hash)
            {
                Ok(row) => match reader_navigation::resolve(row, request) {
                    Ok(context) => Json(context).into_response(),
                    Err(reason) => error(reason),
                },
                Err(reason) => {
                    tracing::error!(error = %reason, "reader navigation query failed");
                    error(NavigationError::Database)
                }
            },
            Err(reason) => error(reason),
        },
        Err(_) => error(NavigationError::InvalidRequest),
    };
    let mut response = result;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
