use crate::audio_encoding;
use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    pub ext: Option<String>,
    pub format: Option<String>,
    pub cache: Option<bool>,
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

    if query.cache == Some(true) {
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
