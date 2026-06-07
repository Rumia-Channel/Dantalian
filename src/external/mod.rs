mod amazon;
mod audio_meta;
mod discogs;
mod isdn;
mod musicbrainz;
pub(crate) mod ndl;

pub use amazon::lookup_amazon_cover_for_jan;
pub use amazon::lookup_isbn;
pub use discogs::lookup_cd_discogs;
pub use isdn::lookup_isdn;
pub use musicbrainz::lookup_cd;

pub(crate) use self::save_uploaded_audio::save_uploaded_audio;

mod save_uploaded_audio {
    use base64::Engine;
    use sha3::{Digest, Sha3_256};

    pub(crate) const AUDIO_MAX_BYTES: usize = 100 * 1024 * 1024;

    pub(crate) fn save_uploaded_audio(
        bytes: &[u8],
        original_name: &str,
        audio_dir: &str,
    ) -> Result<(String, String), String> {
        if bytes.len() > AUDIO_MAX_BYTES {
            return Err(format!(
                "Audio too large: {} bytes (max {} MB)",
                bytes.len(),
                AUDIO_MAX_BYTES / 1024 / 1024
            ));
        }

        let ext = std::path::Path::new(original_name)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "webm"))
            .ok_or_else(|| format!("Unsupported audio extension: {}", original_name))?;

        let hash = Sha3_256::digest(bytes);
        let hash_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);
        let save_name = format!("{}.{}", hash_b64, ext);

        std::fs::create_dir_all(audio_dir)
            .map_err(|e| format!("Failed to create audio dir: {}", e))?;

        let save_path = std::path::Path::new(audio_dir).join(&save_name);
        std::fs::write(&save_path, bytes).map_err(|e| format!("Failed to save audio: {}", e))?;

        Ok((save_name, ext.to_string()))
    }
}

use base64::Engine;
use reqwest::Client;
use sha3::{Digest, Sha3_256};
use tracing::debug;

pub(crate) async fn download_image(
    client: &Client,
    url: &str,
    images_dir: &str,
    extra_headers: &[(&str, &str)],
) -> Result<String, String> {
    if url.contains("musicbrainz.org") || url.contains("coverartarchive.org") {
        musicbrainz::wait_metabrainz_rate_limit().await;
    }

    let mut req = client
        .get(url)
        .header("User-Agent", ua_generator::ua::spoof_ua());
    for (k, v) in extra_headers {
        req = req.header(*k, *v);
    }
    let response = req
        .send()
        .await
        .map_err(|e| format!("Image download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Image download status {}", response.status()));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let ext = match content_type.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "jpg",
    };

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Image read failed: {}", e))?;

    let hash = Sha3_256::digest(&bytes);
    let filename = format!(
        "{}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash),
        ext
    );
    let filepath = std::path::Path::new(images_dir).join(&filename);

    if !filepath.exists() {
        std::fs::write(&filepath, &bytes).map_err(|e| format!("Failed to save image: {}", e))?;
        debug!(%url, %filename, "Image saved");
    } else {
        debug!(%url, %filename, "Image already exists");
    }

    Ok(filename)
}

