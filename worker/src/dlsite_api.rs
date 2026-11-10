use crate::{
    external_api::{fetch_with_timeout, read_bytes_limited, read_text_limited},
    wasabi::{WasabiConfig, WasabiStorage},
};
use dantalian::dlsite;
use dantalian::ports::object_storage::{ObjectKind, object_key};
use sha2::{Digest, Sha256};
use worker::{Fetch, Headers, Method, Request, RequestInit, Result, RouteContext};

pub async fn register(mut req: Request, ctx: RouteContext<()>) -> Result<worker::Response> {
    let request = match crate::error::parse_json::<dlsite::DlsiteRegisterRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let id = match dlsite::normalize_product_id(Some(&request.dlsite_id)) {
        Ok(Some(id)) => id,
        Ok(None) => return Ok(crate::error::bad_request("DLsite ID is required")),
        Err(message) => return Ok(crate::error::bad_request(message)),
    };
    match request.media_type {
        dlsite::DlsiteMediaType::Book => crate::book_api::register_dlsite(id, ctx).await,
        kind => crate::cd_api::register_dlsite(&id, kind.as_str(), ctx).await,
    }
}
async fn get(url: &str) -> Result<worker::Response> {
    let headers = Headers::new();
    headers.set("User-Agent", "Mozilla/5.0 Dantalian")?;
    headers.set("Accept-Language", "ja-JP,ja;q=0.9")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(url, &init)?;
    let response = fetch_with_timeout(Fetch::Request(request), "DLsite").await?;
    if response.status_code() != 404 && !(200..300).contains(&response.status_code()) {
        return Err(worker::Error::from(format!(
            "DLsite returned HTTP {}",
            response.status_code()
        )));
    }
    Ok(response)
}

pub(crate) async fn lookup(ctx: &RouteContext<()>, id: &str) -> Result<Option<serde_json::Value>> {
    let info = get(&dlsite::info_url(id)).await?;
    if info.status_code() == 404 {
        return Ok(None);
    }
    let info = read_text_limited(info, "DLsite product").await?;
    let info: serde_json::Value = serde_json::from_str(&info).map_err(|error| {
        worker::Error::from(format!("Invalid DLsite product response: {error}"))
    })?;
    if !info.get(id).is_some_and(serde_json::Value::is_object) {
        return Ok(None);
    }
    let html = get(&dlsite::work_url(id)).await?;
    if html.status_code() == 404 {
        return Ok(None);
    }
    let html = read_text_limited(html, "DLsite page").await?;
    let Some(mut work) = dlsite::parse_work(id, &info, &html).map_err(worker::Error::from)? else {
        return Ok(None);
    };
    if let Some(url) = work.cover_url.take() {
        match save_cover(ctx, &url).await {
            Ok(name) => work.cover_url = Some(name),
            Err(error) => worker::console_warn!("DLsite cover download failed for {id}: {error}"),
        }
    }
    let authors = std::mem::take(&mut work.authors)
        .into_iter()
        .map(|name| serde_json::json!({"name":name}))
        .collect::<Vec<_>>();
    let mut body =
        serde_json::to_value(work).map_err(|error| worker::Error::from(error.to_string()))?;
    body["_ndl_authors"] = serde_json::Value::Array(authors);
    body["media_type"] = serde_json::Value::String("book".to_owned());
    Ok(Some(body))
}

async fn save_cover(ctx: &RouteContext<()>, url: &str) -> Result<String> {
    let response = get(url).await?;
    if response.status_code() == 404 {
        return Err(worker::Error::from("DLsite cover not found"));
    }
    let bytes = read_bytes_limited(response, "DLsite cover", 10 * 1024 * 1024).await?;
    let (extension, content_type) = if bytes.starts_with(b"\x89PNG") {
        ("png", "image/png")
    } else if bytes.starts_with(b"RIFF") {
        ("webp", "image/webp")
    } else {
        ("jpg", "image/jpeg")
    };
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hash = String::with_capacity(64);
    for byte in Sha256::digest(&bytes) {
        hash.push(char::from(HEX[usize::from(byte >> 4)]));
        hash.push(char::from(HEX[usize::from(byte & 15)]));
    }
    let config = WasabiConfig::from_env(&ctx.env).await?;
    let key = object_key(
        config.prefix.as_deref(),
        ObjectKind::CoverImage,
        &hash,
        extension,
    )
    .map_err(|error| worker::Error::from(error.to_string()))?;
    WasabiStorage::new(config)
        .put_object(&key, content_type, &bytes)
        .await
        .map_err(|error| worker::Error::from(error.to_string()))?;
    Ok(format!("{hash}.{extension}"))
}
