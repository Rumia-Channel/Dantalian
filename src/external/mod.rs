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

pub(crate) fn normalize_publish_date(raw: Option<&str>) -> Option<String> {
    let normalized: String = raw?
        .trim()
        .chars()
        .map(|ch| match ch {
            '０'..='９' => char::from_u32(ch as u32 - '０' as u32 + '0' as u32).unwrap(),
            '－' => '-',
            '／' => '/',
            '．' => '.',
            _ => ch,
        })
        .collect();
    let s = normalized.trim();
    if s.is_empty() {
        return None;
    }

    if s.chars().all(|ch| ch.is_ascii_digit()) {
        return match s.len() {
            4 => normalize_date_parts(&[&s[0..4]]),
            6 => normalize_date_parts(&[&s[0..4], &s[4..6]]),
            8 => normalize_date_parts(&[&s[0..4], &s[4..6], &s[6..8]]),
            _ => None,
        };
    }

    let separated = s.replace('年', "-").replace('月', "-").replace('日', "");
    let parts: Vec<&str> = separated
        .split(|ch| matches!(ch, '-' | '/' | '.'))
        .filter(|part| !part.is_empty())
        .collect();
    normalize_date_parts(&parts)
}

pub(crate) fn normalize_publish_date_input(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = raw.map(str::trim) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    normalize_publish_date(Some(value))
        .map(Some)
        .ok_or_else(|| "日付は YYYY-MM-DD または YYYY-MM-NN 形式で入力してください".to_string())
}

fn normalize_date_parts(parts: &[&str]) -> Option<String> {
    let year = parse_year(parts.first().copied()?)?;
    match parts.len() {
        1 => Some(format!("{:04}-NN-NN", year)),
        2 => {
            let month = parts[1].to_ascii_uppercase();
            if month == "NN" {
                Some(format!("{:04}-NN-NN", year))
            } else {
                Some(format!("{:04}-{:02}-NN", year, parse_month(&month)?))
            }
        }
        3 => {
            let month = parts[1].to_ascii_uppercase();
            let day = parts[2].to_ascii_uppercase();
            if month == "NN" && day == "NN" {
                return Some(format!("{:04}-NN-NN", year));
            }
            let month = parse_month(&month)?;
            if day == "NN" {
                return Some(format!("{:04}-{:02}-NN", year, month));
            }
            let day = day.parse::<u32>().ok()?;
            chrono::NaiveDate::from_ymd_opt(year, month, day)
                .map(|date| date.format("%Y-%m-%d").to_string())
        }
        _ => None,
    }
}

fn parse_year(value: &str) -> Option<i32> {
    if value.len() != 4 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let year = value.parse::<i32>().ok()?;
    (1900..=2999).contains(&year).then_some(year)
}

fn parse_month(value: &str) -> Option<u32> {
    let month = value.parse::<u32>().ok()?;
    (1..=12).contains(&month).then_some(month)
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
