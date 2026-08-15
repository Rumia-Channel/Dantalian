mod amazon;
pub mod audio_meta;
mod discogs;
mod isdn;
mod musicbrainz;
pub(crate) mod ndl;

pub use amazon::lookup_isbn;
pub use amazon::{lookup_amazon_cover_for_jan, lookup_amazon_title_for_jan};
pub use discogs::lookup_cd_discogs;
pub use isdn::lookup_isdn;
pub use musicbrainz::{lookup_cd, lookup_cd_by_release_id, search_cd_candidates_by_title};

pub(crate) use crate::application::publish_date::{
    normalize_publish_date, normalize_publish_date_input,
};

pub(crate) use self::save_uploaded_audio::save_uploaded_audio;
pub(crate) use self::save_uploaded_audio::save_uploaded_audio_path;
pub(crate) use self::save_uploaded_file::save_uploaded_file;
pub(crate) use self::save_uploaded_file::save_uploaded_file_path;

mod save_uploaded_audio {
    use base64::Engine;
    use sha3::{Digest, Sha3_256};
    use std::io::Read;

    fn audio_extension(original_name: &str) -> Result<String, String> {
        std::path::Path::new(original_name)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| {
                matches!(
                    s.as_str(),
                    "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "webm"
                )
            })
            .ok_or_else(|| format!("Unsupported audio extension: {}", original_name))
    }

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

        let ext = audio_extension(original_name)?;

        let hash = Sha3_256::digest(bytes);
        let hash_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);
        let save_name = format!("{}.{}", hash_b64, ext);

        std::fs::create_dir_all(audio_dir)
            .map_err(|e| format!("Failed to create audio dir: {}", e))?;

        let save_path = std::path::Path::new(audio_dir).join(&save_name);
        std::fs::write(&save_path, bytes).map_err(|e| format!("Failed to save audio: {}", e))?;

        Ok((save_name, ext.to_string()))
    }

    pub(crate) fn save_uploaded_audio_path(
        source_path: &std::path::Path,
        original_name: &str,
        audio_dir: &str,
        max_bytes: usize,
    ) -> Result<(String, String), String> {
        let ext = audio_extension(original_name)?;
        let size = std::fs::metadata(source_path)
            .map_err(|e| format!("Failed to inspect uploaded audio: {}", e))?
            .len();
        if size > max_bytes as u64 {
            return Err(format!(
                "Audio too large: {} bytes (max {} MB)",
                size,
                max_bytes / 1024 / 1024
            ));
        }

        let mut input = std::fs::File::open(source_path)
            .map_err(|e| format!("Failed to open uploaded audio: {}", e))?;
        let mut hasher = Sha3_256::new();
        let mut buffer = [0u8; 1024 * 1024];
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|e| format!("Failed to hash uploaded audio: {}", e))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let hash_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());
        let save_name = format!("{}.{}", hash_b64, ext);
        std::fs::create_dir_all(audio_dir)
            .map_err(|e| format!("Failed to create audio dir: {}", e))?;
        let save_path = std::path::Path::new(audio_dir).join(&save_name);
        std::fs::copy(source_path, &save_path)
            .map_err(|e| format!("Failed to save audio: {}", e))?;
        Ok((save_name, ext))
    }
}

mod save_uploaded_file {
    use base64::Engine;
    use sha3::{Digest, Sha3_256};
    use std::io::Read;

    fn file_extension(original_name: &str) -> Result<String, String> {
        std::path::Path::new(original_name)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| matches!(s.as_str(), "epub" | "pdf" | "zip"))
            .ok_or_else(|| format!("Unsupported file extension: {}", original_name))
    }

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

        let ext = file_extension(original_name)?;

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

    pub(crate) fn save_uploaded_file_path(
        source_path: &std::path::Path,
        original_name: &str,
        epubs_dir: &str,
        max_bytes: usize,
    ) -> Result<(String, String), String> {
        let ext = file_extension(original_name)?;
        let size = std::fs::metadata(source_path)
            .map_err(|e| format!("Failed to inspect uploaded file: {}", e))?
            .len();
        if size > max_bytes as u64 {
            return Err(format!(
                "File too large: {} bytes (max {} MB)",
                size,
                max_bytes / 1024 / 1024
            ));
        }

        let mut input = std::fs::File::open(source_path)
            .map_err(|e| format!("Failed to open uploaded file: {}", e))?;
        let mut header = [0u8; 5];
        let header_len = input
            .read(&mut header)
            .map_err(|e| format!("Failed to inspect uploaded file: {}", e))?;
        match ext.as_str() {
            "epub" | "zip" => {
                if header_len < 4
                    || &header[0..2] != b"PK"
                    || !matches!(&header[2..4], [0x03, 0x04] | [0x05, 0x06] | [0x07, 0x08])
                {
                    return Err("File is not a valid ZIP archive".to_string());
                }
            }
            "pdf" if header_len < 5 || &header != b"%PDF-" => {
                return Err("File is not a valid PDF".to_string());
            }
            _ => {}
        }

        let mut input = std::fs::File::open(source_path)
            .map_err(|e| format!("Failed to open uploaded file: {}", e))?;
        let mut hasher = Sha3_256::new();
        let mut buffer = [0u8; 1024 * 1024];
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|e| format!("Failed to hash uploaded file: {}", e))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let hash_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());
        let save_name = format!("{}.{}", hash_b64, ext);
        std::fs::create_dir_all(epubs_dir)
            .map_err(|e| format!("Failed to create epubs dir: {}", e))?;
        let save_path = std::path::Path::new(epubs_dir).join(&save_name);
        std::fs::copy(source_path, &save_path)
            .map_err(|e| format!("Failed to save file: {}", e))?;
        Ok((save_name, ext))
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

#[cfg(test)]
mod tests {
    use super::{normalize_publish_date, normalize_publish_date_input};

    #[test]
    fn normalizes_month_only_dates_without_inventing_a_day() {
        assert_eq!(
            normalize_publish_date(Some("2024.5")),
            Some("2024-05-NN".to_string())
        );
        assert_eq!(
            normalize_publish_date(Some("2024.05")),
            Some("2024-05-NN".to_string())
        );
        assert_eq!(
            normalize_publish_date(Some("2024年5月")),
            Some("2024-05-NN".to_string())
        );
        assert_eq!(
            normalize_publish_date(Some("２０２６年２月１３日")),
            Some("2026-02-13".to_string())
        );
    }

    #[test]
    fn normalizes_full_and_year_only_dates() {
        assert_eq!(
            normalize_publish_date(Some("2024/5/2")),
            Some("2024-05-02".to_string())
        );
        assert_eq!(
            normalize_publish_date(Some("2024")),
            Some("2024-NN-NN".to_string())
        );
        assert_eq!(
            normalize_publish_date(Some("2024-05-NN")),
            Some("2024-05-NN".to_string())
        );
    }

    #[test]
    fn rejects_invalid_date_input() {
        assert!(normalize_publish_date_input(Some("2024-13")).is_err());
        assert!(normalize_publish_date_input(Some("2024-02-31")).is_err());
        assert_eq!(normalize_publish_date_input(Some("  ")), Ok(None));
    }
}
