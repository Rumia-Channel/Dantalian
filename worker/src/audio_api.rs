use dantalian::{
    application::error::AppError,
    ports::object_storage::{AudioCodec, ObjectKind, ObjectStorage, object_key},
};
use serde::Serialize;
use worker::{Request, Response, Result, RouteContext};

use crate::{
    error::{bad_request, error_response},
    wasabi::{WasabiConfig, WasabiStorage},
};

const AUDIO_EXTERNAL_PROCESSING_STATUS: u16 = 501;

#[derive(Debug, Serialize)]
struct AudioVariantAvailability {
    available: bool,
    size_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
struct AudioPlayabilityResponse {
    original: AudioVariantAvailability,
    opus: AudioVariantAvailability,
    aac: AudioVariantAvailability,
    preferred_format: Option<&'static str>,
}

/// Audio transcoding is intentionally outside the Worker boundary.
///
/// The native implementation performs full-input decoding and buffering. The
/// Worker keeps this route as an explicit contract so callers can dispatch the
/// job to the external processor instead of silently attempting it in WASM.
pub async fn encode(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::from_json(&serde_json::json!({
        "error": "audio processing requires the external processor",
        "code": "audio_processing_external_required",
    }))
    .map(|response| response.with_status(AUDIO_EXTERNAL_PROCESSING_STATUS))
}

fn normalize_extension(value: &str) -> Option<String> {
    let extension = value.trim().trim_start_matches('.');
    if extension.is_empty()
        || extension.len() > 12
        || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(extension.to_ascii_lowercase())
}

async fn inspect_audio_variant(
    storage: &WasabiStorage,
    key: &str,
) -> std::result::Result<AudioVariantAvailability, AppError> {
    match storage.head(key).await {
        Ok(metadata) => {
            let size_bytes = metadata.content_length;
            Ok(AudioVariantAvailability {
                available: size_bytes.is_some_and(|size| size > 0),
                size_bytes,
            })
        }
        Err(AppError::NotFound) => Ok(AudioVariantAvailability {
            available: false,
            size_bytes: None,
        }),
        Err(error) => Err(error),
    }
}

pub async fn playability(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let file_hash = match ctx.param("file_hash") {
        Some(value) if !value.is_empty() => value,
        _ => return Ok(bad_request("missing file hash")),
    };
    let extension = req
        .url()?
        .query_pairs()
        .find(|(key, _)| key == "ext")
        .and_then(|(_, value)| normalize_extension(&value));
    let Some(extension) = extension else {
        return Ok(bad_request("missing or invalid audio extension"));
    };

    let config = match WasabiConfig::from_env(&ctx.env).await {
        Ok(config) => config,
        Err(_) => {
            return error_response(AppError::Storage(
                "Wasabi storage is not configured".to_string(),
            ));
        }
    };
    let storage = WasabiStorage::new(config.clone());
    let original_key = match object_key(
        config.prefix.as_deref(),
        ObjectKind::OriginalAudio,
        file_hash,
        &extension,
    ) {
        Ok(key) => key,
        Err(error) => return error_response(error),
    };
    let opus_key = match object_key(
        config.prefix.as_deref(),
        ObjectKind::EncodedAudio {
            codec: AudioCodec::Opus,
        },
        file_hash,
        "opus",
    ) {
        Ok(key) => key,
        Err(error) => return error_response(error),
    };
    let aac_key = match object_key(
        config.prefix.as_deref(),
        ObjectKind::EncodedAudio {
            codec: AudioCodec::Aac,
        },
        file_hash,
        "aac",
    ) {
        Ok(key) => key,
        Err(error) => return error_response(error),
    };

    let original = match inspect_audio_variant(&storage, &original_key).await {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let opus = match inspect_audio_variant(&storage, &opus_key).await {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let aac = match inspect_audio_variant(&storage, &aac_key).await {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let preferred_format = if opus.available {
        Some("opus")
    } else if aac.available {
        Some("aac")
    } else if original.available {
        Some("original")
    } else {
        None
    };

    Response::from_json(&AudioPlayabilityResponse {
        original,
        opus,
        aac,
        preferred_format,
    })
}
