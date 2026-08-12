use crate::{
    AppState,
    adapters::native_series::NativeSeriesRepository,
    api::error::{ApiError, error_response},
    application::series::SeriesService,
    domain::series::{CreateSeries, RenameSeries, Series},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateSeries>,
) -> Result<(StatusCode, Json<Series>), ApiError> {
    let service = SeriesService::new(NativeSeriesRepository::new(state.db));
    service
        .create(&request.name)
        .await
        .map(|series| (StatusCode::CREATED, Json(series)))
        .map_err(error_response)
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Series>>, ApiError> {
    let service = SeriesService::new(NativeSeriesRepository::new(state.db));
    service.list().await.map(Json).map_err(error_response)
}

pub async fn rename(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<RenameSeries>,
) -> Result<StatusCode, ApiError> {
    let service = SeriesService::new(NativeSeriesRepository::new(state.db));
    service
        .rename(id, &request.name)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let service = SeriesService::new(NativeSeriesRepository::new(state.db));

    service
        .delete(id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}
#[cfg(test)]
mod tests {
    use crate::{AppState, api};
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use reqwest::Client;
    use std::sync::Arc;
    use tokio::sync::Notify;
    use tower::ServiceExt;

    fn app() -> Router {
        let state = AppState {
            db: crate::db::Db::new(":memory:").unwrap(),
            client: Client::new(),
            client_ipv4: Client::new(),
            images_dir: Arc::new(String::new()),
            audio_dir: Arc::new(String::new()),
            audio_encoding_notify: Arc::new(Notify::new()),
            epubs_dir: Arc::new(String::new()),
            uploads_dir: Arc::new(String::new()),
            discogs_token: String::new(),
            musicbrainz_contact: String::new(),
        };
        Router::new().nest("/api", api::routes()).with_state(state)
    }

    fn json_request(method: axum::http::Method, uri: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn series_api_contract() {
        let app = app();

        let response = app
            .clone()
            .oneshot(json_request(
                axum::http::Method::POST,
                "/api/series",
                r#"{"name":""}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error"],
            "Series name is required"
        );

        let response = app
            .clone()
            .oneshot(json_request(
                axum::http::Method::POST,
                "/api/series",
                r#"{"name":"Contract"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["name"],
            "Contract"
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/series")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(
            serde_json::from_slice::<serde_json::Value>(&body)
                .unwrap()
                .is_array()
        );

        let response = app
            .clone()
            .oneshot(json_request(
                axum::http::Method::PUT,
                "/api/series/1",
                r#"{"name":"Renamed"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = app
            .clone()
            .oneshot(json_request(
                axum::http::Method::PUT,
                "/api/series/999",
                r#"{"name":"Missing"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = app
            .clone()
            .oneshot(json_request(
                axum::http::Method::PUT,
                "/api/series/invalid",
                r#"{"name":"Invalid"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(axum::http::Method::DELETE)
                    .uri("/api/series/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = app
            .oneshot(
                Request::builder()
                    .method(axum::http::Method::DELETE)
                    .uri("/api/series/1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
}
