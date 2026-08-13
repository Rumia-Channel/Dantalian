use dantalian::{
    application::error::AppError,
    ports::{
        audio_jobs::{AudioJobQueue, AudioJobRequest},
        object_storage::{AudioCodec, ObjectStorage, validate_object_key},
    },
};
use serde::Deserialize;
use worker::{Request, Response, Result, RouteContext};

use crate::{
    audio_job_repository::D1AudioJobQueue,
    error::{bad_request, error_response, parse_json},
    wasabi::{WasabiConfig, WasabiStorage},
};

const DEFAULT_BITRATE_KBPS: u32 = 192;

#[derive(Debug, Deserialize)]
pub struct CreateAudioJobRequest {
    pub input_object_key: String,
    pub output_object_key: String,
    pub codec: AudioCodec,
    pub bitrate_kbps: Option<u32>,
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CreateAudioJobRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let request = match validate_request(request) {
        Ok(request) => request,
        Err(error) => return error_response(error),
    };

    let config = match WasabiConfig::from_env(&ctx.env) {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let storage = WasabiStorage::new(config);
    match storage.exists(&request.input_object_key).await {
        Ok(true) => {}
        Ok(false) => return error_response(AppError::NotFound),
        Err(error) => return error_response(error),
    }
    match storage.exists(&request.output_object_key).await {
        Ok(true) => {
            return error_response(AppError::Conflict(
                "audio job output object already exists".to_string(),
            ));
        }
        Ok(false) => {}
        Err(error) => return error_response(error),
    }

    let db = ctx.d1("DB")?;
    let queue = D1AudioJobQueue::new(&db);
    let job = match queue.submit(request).await {
        Ok(job) => job,
        Err(error) => return error_response(error),
    };
    Response::from_json(&job).map(|response| response.with_status(202))
}

pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let job_id = match job_id(&ctx) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let queue = D1AudioJobQueue::new(&db);
    match queue.get(&job_id).await {
        Ok(job) => Response::from_json(&job),
        Err(error) => error_response(error),
    }
}

fn validate_request(request: CreateAudioJobRequest) -> Result<AudioJobRequest, AppError> {
    validate_object_key(&request.input_object_key)?;
    validate_object_key(&request.output_object_key)?;
    if request.input_object_key == request.output_object_key {
        return Err(AppError::Validation(
            "audio job input and output keys must differ".to_string(),
        ));
    }
    let expected_extension = match request.codec {
        AudioCodec::Opus => ".opus",
        AudioCodec::Aac => ".aac",
    };
    if !request
        .output_object_key
        .to_ascii_lowercase()
        .ends_with(expected_extension)
    {
        return Err(AppError::Validation(format!(
            "audio job output key must end with {expected_extension}"
        )));
    }
    let bitrate_kbps = request.bitrate_kbps.unwrap_or(DEFAULT_BITRATE_KBPS);
    if !(8..=512).contains(&bitrate_kbps) {
        return Err(AppError::Validation(
            "audio bitrate must be between 8 and 512 kbps".to_string(),
        ));
    }
    Ok(AudioJobRequest {
        input_object_key: request.input_object_key,
        output_object_key: request.output_object_key,
        codec: request.codec,
        bitrate_kbps,
    })
}

fn job_id(ctx: &RouteContext<()>) -> std::result::Result<String, Response> {
    let value = ctx.param("id").map(String::as_str).unwrap_or_default();
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad_request("invalid audio job id"));
    }
    Ok(value.to_string())
}
