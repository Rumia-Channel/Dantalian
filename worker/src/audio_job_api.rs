use dantalian::{
    application::{
        audio_jobs::{AudioJobService, prepare_request},
        error::AppError,
    },
    ports::object_storage::AudioCodec,
};
use serde::Deserialize;
use worker::{Request, Response, Result, RouteContext};

use crate::{
    audio_job_repository::D1AudioJobQueue,
    error::{bad_request, error_response, parse_json},
    wasabi::{WasabiConfig, WasabiStorage},
};

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
    let request = match prepare_request(
        request.input_object_key,
        request.output_object_key,
        request.codec,
        request.bitrate_kbps,
    ) {
        Ok(request) => request,
        Err(error) => return error_response(error),
    };

    let config = match WasabiConfig::from_env(&ctx.env) {
        Ok(config) => config,
        Err(error) => return error_response(AppError::Storage(error.to_string())),
    };
    let storage = WasabiStorage::new(config);
    let db = ctx.d1("DB")?;
    let queue = D1AudioJobQueue::new(&db);
    let service = AudioJobService::new(queue);
    let job = match service.submit(&storage, request).await {
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
    let service = AudioJobService::new(queue);
    match service.get(&job_id).await {
        Ok(job) => Response::from_json(&job),
        Err(error) => error_response(error),
    }
}

fn job_id(ctx: &RouteContext<()>) -> std::result::Result<String, Response> {
    let value = ctx.param("id").map(String::as_str).unwrap_or_default();
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad_request("invalid audio job id"));
    }
    Ok(value.to_string())
}
