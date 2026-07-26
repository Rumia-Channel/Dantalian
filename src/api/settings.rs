use axum::{Json, extract::State};
use std::collections::HashMap;

pub async fn get_settings(State(state): State<crate::AppState>) -> Json<HashMap<String, String>> {
    let settings = state.db.get_all_settings();
    Json(settings)
}

pub async fn update_settings(
    State(state): State<crate::AppState>,
    Json(settings): Json<HashMap<String, String>>,
) -> Result<Json<HashMap<String, String>>, axum::http::StatusCode> {
    state
        .db
        .set_settings(&settings)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let current = state.db.get_all_settings();
    Ok(Json(current))
}
