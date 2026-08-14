use dantalian::{
    application::{
        audio_jobs::{AudioJobService, prepare_request},
        error::AppError,
    },
    ports::{
        audio_jobs::{AudioJobClaim, AudioJobDispatchMessage, AudioJobFailure, AudioJobStatus},
        object_storage::AudioCodec,
    },
};
use serde::Deserialize;
use worker::{Env, Request, Response, Result, RouteContext};

use crate::{
    audio_job_repository::D1AudioJobRepository,
    error::{bad_request, error_response, parse_json},
    wasabi::{WasabiConfig, WasabiStorage},
};

const QUEUE_MESSAGE_VERSION: u8 = 1;
const RECOVERY_BATCH_SIZE: u32 = 100;

#[derive(Debug, Deserialize)]
pub struct CreateAudioJobRequest {
    pub input_object_key: String,
    pub output_object_key: String,
    pub codec: AudioCodec,
    pub bitrate_kbps: Option<u32>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaimRequest {
    processor_id: String,
    lease_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RenewLeaseRequest {
    claim: AudioJobClaim,
    lease_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CompleteRequest {
    claim: AudioJobClaim,
}

#[derive(Debug, Deserialize)]
struct FailRequest {
    claim: AudioJobClaim,
    failure: AudioJobFailure,
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
        request.idempotency_key,
    ) {
        Ok(request) => request,
        Err(error) => return error_response(error),
    };

    let storage = match storage(&ctx).await {
        Ok(storage) => storage,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    let job = match service.submit(&storage, request).await {
        Ok(job) => job,
        Err(error) => return error_response(error),
    };
    if let Err(error) = dispatch(&ctx.env, &job.id).await {
        worker::console_error!("audio job dispatch deferred for {}: {error}", job.id);
    }
    Response::from_json(&job).map(|response| response.with_status(202))
}

pub async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let job_id = match job_id(&ctx) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    match service.get(&job_id).await {
        Ok(job) => Response::from_json(&job),
        Err(error) => error_response(error),
    }
}

pub async fn retry(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let job_id = match job_id(&ctx) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    match service.retry(&job_id).await {
        Ok(job) => {
            if let Err(error) = dispatch(&ctx.env, &job.id).await {
                worker::console_error!("audio job retry dispatch deferred for {}: {error}", job.id);
            }
            Response::from_json(&job).map(|response| response.with_status(202))
        }
        Err(error) => error_response(error),
    }
}

pub async fn claim(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<ClaimRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    match service
        .claim_next(
            &request.processor_id,
            request.lease_seconds.unwrap_or_default(),
        )
        .await
    {
        Ok(Some(claim)) => Response::from_json(&claim),
        Ok(None) => Ok(Response::empty()?.with_status(204)),
        Err(error) => error_response(error),
    }
}

pub async fn claim_by_id(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let job_id = match job_id(&ctx) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let request = match parse_json::<ClaimRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    let db = ctx.d1("DB")?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    match service
        .claim_by_id(
            &job_id,
            &request.processor_id,
            request.lease_seconds.unwrap_or_default(),
        )
        .await
    {
        Ok(Some(claim)) => Response::from_json(&claim),
        Ok(None) => Ok(Response::empty()?.with_status(204)),
        Err(error) => error_response(error),
    }
}

pub async fn renew(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<RenewLeaseRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    if let Err(response) = validate_claim_path(&ctx, &request.claim) {
        return Ok(response);
    }
    let db = ctx.d1("DB")?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    match service
        .renew_lease(&request.claim, request.lease_seconds.unwrap_or_default())
        .await
    {
        Ok(claim) => Response::from_json(&claim),
        Err(error) => error_response(error),
    }
}

pub async fn complete(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<CompleteRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    if let Err(response) = validate_claim_path(&ctx, &request.claim) {
        return Ok(response);
    }
    let storage = match storage(&ctx).await {
        Ok(storage) => storage,
        Err(error) => return error_response(error),
    };
    let db = ctx.d1("DB")?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    match service.complete(&storage, &request.claim).await {
        Ok(job) => Response::from_json(&job),
        Err(error) => error_response(error),
    }
}

pub async fn fail(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let request = match parse_json::<FailRequest>(&mut req).await {
        Ok(request) => request,
        Err(response) => return Ok(response),
    };
    if let Err(response) = validate_claim_path(&ctx, &request.claim) {
        return Ok(response);
    }
    let db = ctx.d1("DB")?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    match service.fail(&request.claim, request.failure).await {
        Ok(job) => {
            if job.status == AudioJobStatus::Queued {
                if let Err(error) = dispatch(&ctx.env, &job.id).await {
                    worker::console_error!(
                        "audio job retry dispatch deferred for {}: {error}",
                        job.id
                    );
                }
            }
            Response::from_json(&job)
        }
        Err(error) => error_response(error),
    }
}

async fn storage(ctx: &RouteContext<()>) -> Result<WasabiStorage, AppError> {
    let config = WasabiConfig::from_env(&ctx.env)
        .await
        .map_err(|error| AppError::Storage(error.to_string()))?;
    Ok(WasabiStorage::new(config))
}

fn job_id(ctx: &RouteContext<()>) -> std::result::Result<String, Response> {
    let value = ctx.param("id").map(String::as_str).unwrap_or_default();
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad_request("invalid audio job id"));
    }
    Ok(value.to_string())
}

fn validate_claim_path(
    ctx: &RouteContext<()>,
    claim: &AudioJobClaim,
) -> std::result::Result<(), Response> {
    if ctx.param("id").is_some() && job_id(ctx)?.as_str() != claim.job.id {
        return Err(bad_request("claim job id mismatch"));
    }
    Ok(())
}

pub async fn dispatch(env: &Env, job_id: &str) -> Result<(), AppError> {
    let queue = env
        .queue("AUDIO_JOB_QUEUE")
        .map_err(|error| AppError::Internal(format!("audio queue binding unavailable: {error}")))?;
    queue
        .send(AudioJobDispatchMessage {
            version: QUEUE_MESSAGE_VERSION,
            job_id: job_id.to_string(),
        })
        .await
        .map_err(|error| AppError::Internal(format!("audio queue publish failed: {error}")))
}

pub async fn recover_and_dispatch(env: &Env) -> Result<(), AppError> {
    let db = env
        .d1("DB")
        .map_err(|error| AppError::Database(error.to_string()))?;
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    let _ = service.recover_expired().await?;
    for job_id in service.dispatchable_ids(RECOVERY_BATCH_SIZE).await? {
        dispatch(env, &job_id).await?;
    }
    Ok(())
}
