use crate::{
    AppState,
    adapters::native_borrower::NativeBorrowerRepository,
    api::error::{ApiError, error_response},
    application::borrower::BorrowerService,
    domain::borrower::{Borrower, CreateBorrower, UpdateBorrower},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Borrower>>, ApiError> {
    let service = BorrowerService::new(NativeBorrowerRepository::new(state.db));
    service.list().await.map(Json).map_err(error_response)
}

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateBorrower>,
) -> Result<(StatusCode, Json<Borrower>), ApiError> {
    let service = BorrowerService::new(NativeBorrowerRepository::new(state.db));
    service
        .create(&request.name, request.notes.as_deref())
        .await
        .map(|borrower| (StatusCode::CREATED, Json(borrower)))
        .map_err(error_response)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateBorrower>,
) -> Result<StatusCode, ApiError> {
    let service = BorrowerService::new(NativeBorrowerRepository::new(state.db));
    service
        .update(id, request.name.as_deref(), request.notes.as_deref())
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let service = BorrowerService::new(NativeBorrowerRepository::new(state.db));
    service
        .delete(id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(error_response)
}
