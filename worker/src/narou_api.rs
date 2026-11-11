use std::collections::HashMap;

use dantalian::narou::{Config, READ_API_PATH};
use serde::Deserialize;
use worker::{Request, Response, Result, RouteContext};

use crate::d1_read::{self, D1Prepare};

pub(crate) fn validate_url(value: &str) -> std::result::Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "narou.rs の URL が不正です".to_string())?;
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

#[derive(Deserialize)]
struct SettingRow {
    key: String,
    value: String,
}

pub(crate) async fn load_config<D: D1Prepare>(db: &D) -> Result<Config> {
    let rows = db.prepare("SELECT key,value FROM settings WHERE key IN ('narou.url','narou.tags','narou.use_favorites')")
        .all().await?.results::<SettingRow>()?;
    let settings: HashMap<_, _> = rows.into_iter().map(|row| (row.key, row.value)).collect();
    let mut config = Config::from_settings(&settings).map_err(worker::Error::RustError)?;
    if !config.url.is_empty() {
        config.url = String::from(validate_url(&config.url).map_err(worker::Error::RustError)?);
        while config.url.ends_with('/') {
            config.url.pop();
        }
    }
    Ok(config)
}

fn private_response(mut response: Response) -> Result<Response> {
    response.headers_mut().set("Cache-Control", "no-store")?;
    response
        .headers_mut()
        .set("Referrer-Policy", "no-referrer")?;
    Ok(response)
}

pub async fn connection(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.env.d1("DB")?;
    let db = d1_read::read_db(&req, &db)?;
    private_response(Response::from_json(
        &load_config(&db).await?.into_connection(),
    )?)
}

pub async fn epub(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let Some(id) = ctx
        .param("id")
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|id| *id >= 0)
    else {
        return private_response(
            Response::from_json(&serde_json::json!({"error":"小説 ID が不正です"}))?
                .with_status(400),
        );
    };
    let db = ctx.env.d1("DB")?;
    let db = d1_read::read_db(&req, &db)?;
    let config = load_config(&db).await?;
    if config.url.is_empty() {
        return private_response(
            Response::from_json(&serde_json::json!({"error":"narou.rs が設定されていません"}))?
                .with_status(404),
        );
    }
    let mut response = Response::empty()?.with_status(302);
    response.headers_mut().set(
        "Location",
        &format!("{}{READ_API_PATH}/novels/{id}/epub", config.url),
    )?;
    private_response(response)
}
