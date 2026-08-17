use crate::audio_encoding;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Json, Redirect};
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    pub ext: Option<String>,
    pub format: Option<String>,
    pub cache: Option<bool>,
    pub wait: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PlayabilityQuery {
    pub ext: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AudioVariantAvailability {
    pub available: bool,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct AudioPlayabilityResponse {
    pub original: AudioVariantAvailability,
    pub opus: AudioVariantAvailability,
    pub aac: AudioVariantAvailability,
    pub preferred_format: Option<&'static str>,
}

pub async fn stream(
    State(state): State<crate::AppState>,
    Path(file_hash): Path<String>,
    Query(query): Query<StreamQuery>,
) -> Redirect {
    let original = Redirect::temporary(&format!("/audio/{}", file_hash));
    let Some(extension) = query
        .ext
        .as_deref()
        .and_then(audio_encoding::normalize_extension)
    else {
        return original;
    };
    let format = match query.format.as_deref() {
        Some("opus") => "opus",
        Some("aac") => "aac",
        _ => return original,
    };
    if query.cache != Some(true)
        && !audio_encoding::AudioDataSaverConfig::load(&state.db).applies_to(&extension)
    {
        return original;
    }
    if !audio_encoding::is_safe_hash(&file_hash) {
        return original;
    }

    if query.cache == Some(true) && query.wait != Some(false) {
        // 明示的なオフラインキャッシュ操作だけは、選択した形式を確実に
        // 保存できるよう、従来どおりこのリクエスト内で生成完了を待つ。
        let audio_dir = state.audio_dir.as_ref().clone();
        let hash_for_log = file_hash.clone();
        let hash_for_encoding = file_hash.clone();
        let result = tokio::task::spawn_blocking(move || {
            audio_encoding::ensure_encoded_variants(&audio_dir, &hash_for_encoding, &extension)
        })
        .await;
        let variants = match result {
            Ok(Ok(variants)) => variants,
            Ok(Err(error)) => {
                tracing::warn!(file_hash = %hash_for_log, "data-saver generation failed: {}", error);
                return original;
            }
            Err(error) => {
                tracing::warn!(file_hash = %hash_for_log, "data-saver task failed: {}", error);
                return original;
            }
        };

        let available = match format {
            "opus" => variants.opus,
            "aac" => variants.aac,
            _ => false,
        };
        if !available {
            return original;
        }
    }

    let encoded_path = audio_encoding::encoded_path(state.audio_dir.as_ref(), &file_hash, format);
    let encoded_exists = tokio::fs::metadata(&encoded_path)
        .await
        .map(|metadata| metadata.is_file())
        .unwrap_or(false);
    if !encoded_exists {
        // 通常の再生ではバックグラウンド生成を待たず、原音を直ちに再生する。
        return original;
    }
    let encoded_name = audio_encoding::encoded_file_name(&file_hash, format);
    Redirect::temporary(&format!("/audio/encoded/{}/{}", format, encoded_name))
}

async fn inspect_audio_variant(path: &FsPath) -> AudioVariantAvailability {
    match tokio::fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() && metadata.len() > 0 => AudioVariantAvailability {
            available: true,
            size_bytes: Some(metadata.len()),
        },
        _ => AudioVariantAvailability {
            available: false,
            size_bytes: None,
        },
    }
}

pub async fn playability(
    State(state): State<crate::AppState>,
    Path(file_hash): Path<String>,
    Query(query): Query<PlayabilityQuery>,
) -> Result<Json<AudioPlayabilityResponse>, StatusCode> {
    query
        .ext
        .as_deref()
        .and_then(audio_encoding::normalize_extension)
        .ok_or(StatusCode::BAD_REQUEST)?;
    if !audio_encoding::is_safe_hash(&file_hash) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let original_path = FsPath::new(state.audio_dir.as_ref()).join(&file_hash);
    let original = inspect_audio_variant(&original_path).await;
    let opus = inspect_audio_variant(&audio_encoding::encoded_path(
        state.audio_dir.as_ref(),
        &file_hash,
        "opus",
    ))
    .await;
    let aac = inspect_audio_variant(&audio_encoding::encoded_path(
        state.audio_dir.as_ref(),
        &file_hash,
        "aac",
    ))
    .await;

    let preferred_format = if opus.available {
        Some("opus")
    } else if aac.available {
        Some("aac")
    } else if original.available {
        Some("original")
    } else {
        None
    };

    Ok(Json(AudioPlayabilityResponse {
        original,
        opus,
        aac,
        preferred_format,
    }))
}
