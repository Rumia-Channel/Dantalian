mod amazon;
pub mod audio_meta;
mod discogs;
mod isdn;
mod musicbrainz;
pub(crate) mod ndl;

pub use amazon::lookup_amazon_cover_for_jan;
pub use amazon::lookup_isbn;
pub use discogs::lookup_cd_discogs;
pub use isdn::lookup_isdn;
pub use musicbrainz::lookup_cd;

pub(crate) fn normalize_publish_date(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    let formats = [
        "%Y-%m-%d", "%Y-%m", "%Y/%m/%d", "%Y/%m", "%Y.%m.%d", "%Y.%m", "%Y%m%d",
    ];
    for fmt in formats {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    if s.len() == 4 {
        if let Ok(y) = s.parse::<i32>() {
            if (1900..=2999).contains(&y) {
                return Some(format!("{:04}-01-01", y));
            }
        }
    }
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        let y: i32 = digits[0..4].parse().ok()?;
        if !(1900..=2999).contains(&y) {
            return None;
        }
        let m: i32 = if digits.len() >= 6 {
            digits[4..6].parse().unwrap_or(1)
        } else {
            1
        };
        let d: i32 = if digits.len() >= 8 {
            digits[6..8].parse().unwrap_or(1)
        } else {
            1
        };
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        return Some(format!("{:04}-{:02}-{:02}", y, m, d));
    }
    None
}

pub(crate) use self::save_uploaded_audio::save_uploaded_audio;
pub(crate) use self::save_uploaded_file::save_uploaded_file;

mod save_uploaded_audio {
    use base64::Engine;
    use sha3::{Digest, Sha3_256};

    pub(crate) fn save_uploaded_audio(
        bytes: &[u8],
        original_name: &str,
        audio_dir: &str,
        max_bytes: usize,
    ) -> Result<(String, String), String> {
        if bytes.len() > max_bytes {
            return Err(format!(
                "Audio too large: {} bytes (max {} MB)",
                bytes.len(),
                max_bytes / 1024 / 1024
            ));
        }

        let ext = std::path::Path::new(original_name)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| {
                matches!(
                    s.as_str(),
                    "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "webm"
                )
            })
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

mod save_uploaded_file {
    use base64::Engine;
    use sha3::{Digest, Sha3_256};

    pub(crate) fn save_uploaded_file(
        bytes: &[u8],
        original_name: &str,
        epubs_dir: &str,
        max_bytes: usize,
    ) -> Result<(String, String), String> {
        if bytes.len() > max_bytes {
            return Err(format!(
                "File too large: {} bytes (max {} MB)",
                bytes.len(),
                max_bytes / 1024 / 1024
            ));
        }

        let ext = std::path::Path::new(original_name)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "epub" | "pdf" | "zip"))
            .ok_or_else(|| format!("Unsupported file extension: {}", original_name))?;

        match ext.as_str() {
            "epub" | "zip" => {
                if bytes.len() < 4 || &bytes[0..2] != b"PK" {
                    return Err("File is not a valid ZIP archive".to_string());
                }
                let ok_signature =
                    matches!(&bytes[2..4], [0x03, 0x04] | [0x05, 0x06] | [0x07, 0x08]);
                if !ok_signature {
                    return Err("File is not a valid ZIP archive".to_string());
                }
            }
            "pdf" => {
                if bytes.len() < 5 || &bytes[0..5] != b"%PDF-" {
                    return Err("File is not a valid PDF".to_string());
                }
            }
            _ => {}
        }

        let hash = Sha3_256::digest(bytes);
        let hash_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);
        let save_name = format!("{}.{}", hash_b64, ext);

        std::fs::create_dir_all(epubs_dir)
            .map_err(|e| format!("Failed to create epubs dir: {}", e))?;

        let save_path = std::path::Path::new(epubs_dir).join(&save_name);
        std::fs::write(&save_path, bytes).map_err(|e| format!("Failed to save file: {}", e))?;

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
