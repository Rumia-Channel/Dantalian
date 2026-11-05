use crate::dlsite;
use base64::Engine;
use reqwest::Client;
use sha3::{Digest, Sha3_256};
use std::time::Duration;

async fn fetch(client: &Client, url: &str, limit: usize) -> Result<Option<Vec<u8>>, String> {
    let mut response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 Dantalian")
        .header("Accept-Language", "ja-JP,ja;q=0.9")
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| format!("DLsite request failed: {error}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    response
        .error_for_status_ref()
        .map_err(|error| format!("DLsite returned an error: {error}"))?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("DLsite body read failed: {error}"))?
    {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("DLsite response exceeds the size limit".to_owned());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Some(bytes))
}

pub(crate) async fn lookup(
    client: &Client,
    id: &str,
    images_dir: &str,
) -> Result<Option<dlsite::DlsiteWork>, String> {
    let Some(info) = fetch(client, &dlsite::info_url(id), 4 * 1024 * 1024).await? else {
        return Ok(None);
    };
    let info = serde_json::from_slice::<serde_json::Value>(&info)
        .map_err(|error| format!("Invalid DLsite product response: {error}"))?;
    if !info.get(id).is_some_and(serde_json::Value::is_object) {
        return Ok(None);
    }
    let Some(html) = fetch(client, &dlsite::work_url(id), 4 * 1024 * 1024).await? else {
        return Ok(None);
    };
    let html = String::from_utf8(html)
        .map_err(|error| format!("Invalid DLsite page encoding: {error}"))?;
    let Some(mut work) = dlsite::parse_work(id, &info, &html)? else {
        return Ok(None);
    };
    let mut cover = None;
    if let Some(url) = work.cover_url.as_deref() {
        match fetch(client, url, 10 * 1024 * 1024).await {
            Ok(Some(bytes)) => {
                let extension = if bytes.starts_with(b"\x89PNG") {
                    "png"
                } else if bytes.starts_with(b"RIFF") {
                    "webp"
                } else {
                    "jpg"
                };
                let name = format!(
                    "{}.{}",
                    base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .encode(Sha3_256::digest(&bytes)),
                    extension
                );
                std::fs::write(std::path::Path::new(images_dir).join(&name), bytes)
                    .map_err(|error| format!("DLsite cover save failed: {error}"))?;
                cover = Some(name);
            }
            Ok(None) => tracing::warn!(%id, "DLsite cover was not found"),
            Err(error) => tracing::warn!(%id, %error, "DLsite cover download failed"),
        }
    }
    work.cover_url = cover;
    Ok(Some(work))
}
