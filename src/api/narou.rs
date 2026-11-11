use crate::{
    AppState,
    narou::{Config, READ_API_PATH},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};

type ApiError = (StatusCode, Json<serde_json::Value>);
fn error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(serde_json::json!({"error":message.into()})))
}

pub(super) fn validate_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "narou.rs の URL が不正です".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "narou.rs URL は認証情報・クエリ・フラグメントを含まない HTTP(S) URL にしてください"
                .into(),
        );
    }
    if url.scheme() != "https"
        && !matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        )
    {
        return Err("narou.rs の直接通信には HTTPS が必要です（ループバックのみ HTTP 可）".into());
    }
    Ok(url)
}

pub(super) fn load_config(db: &crate::db::Db) -> Result<Config, String> {
    let mut config = Config::from_settings(&db.get_all_settings())?;
    if !config.url.is_empty() {
        config.url = String::from(validate_url(&config.url)?);
        while config.url.ends_with('/') {
            config.url.pop();
        }
    }
    Ok(config)
}

fn private_response(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

pub async fn connection(State(state): State<AppState>) -> Result<Response, ApiError> {
    let config =
        load_config(&state.db).map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    Ok(private_response(
        Json(config.into_connection()).into_response(),
    ))
}

pub async fn epub(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    if id <= 0 {
        return Err(error(StatusCode::BAD_REQUEST, "小説 ID が不正です"));
    }
    let config =
        load_config(&state.db).map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    if config.url.is_empty() {
        return Err(error(
            StatusCode::NOT_FOUND,
            "narou.rs が設定されていません",
        ));
    }
    let destination = format!("{}{READ_API_PATH}/novels/{id}/epub", config.url);
    let location = HeaderValue::try_from(destination)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "narou.rs の URL が不正です"))?;
    let mut response = StatusCode::FOUND.into_response();
    response.headers_mut().insert(header::LOCATION, location);
    Ok(private_response(response))
}
