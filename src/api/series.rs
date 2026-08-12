use crate::{
    AppState,
    adapters::native_series::NativeSeriesRepository,
    application::{error::AppError, series::SeriesService},
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
) -> Result<(StatusCode, Json<Series>), (StatusCode, Json<serde_json::Value>)> {
    let service = SeriesService::new(NativeSeriesRepository::new(state.db));
    service
        .create(&request.name)
        .await
        .map(|series| (StatusCode::CREATED, Json(series)))
        .map_err(error_response)
}

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<Series>>, (StatusCode, Json<serde_json::Value>)> {
    let service = SeriesService::new(NativeSeriesRepository::new(state.db));
    service.list().await.map(Json).map_err(error_response)
}

pub async fn rename(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<RenameSeries>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
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
) -> Result<StatusCode, StatusCode> {
    let service = SeriesService::new(NativeSeriesRepository::new(state.db));
    match service.delete(id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(AppError::NotFound) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn error_response(error: AppError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match error {
        AppError::Validation(_) => StatusCode::BAD_REQUEST,
        AppError::NotFound => StatusCode::NOT_FOUND,
        AppError::Conflict(_) => StatusCode::CONFLICT,
        AppError::Database(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(serde_json::json!({ "error": error.to_string() })),
    )
}
