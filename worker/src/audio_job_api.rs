use std::collections::HashSet;

use dantalian::{
    application::{
        audio_jobs::{AudioJobService, DEFAULT_AUDIO_BITRATE_KBPS, prepare_request},
        error::AppError,
    },
    ports::{
        audio_jobs::{AudioJobClaim, AudioJobDispatchMessage, AudioJobFailure, AudioJobStatus},
        object_storage::{AudioCodec, ObjectKind, ObjectStorage, object_key},
    },
};
use serde::Deserialize;
use worker::{D1Type, Env, Request, Response, Result, RouteContext};

use crate::{
    audio_job_repository::{D1AudioJobRepository, OWNER_SCOPE},
    error::{bad_request, error_response, parse_json},
    wasabi::{WasabiConfig, WasabiStorage},
};

const QUEUE_MESSAGE_VERSION: u8 = 1;
const RECOVERY_BATCH_SIZE: u32 = 100;
const DATA_SAVER_DEFAULT_EXTENSIONS: &str = "wav,flac,aiff,alac";

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

#[derive(Debug, Deserialize)]
struct SettingRow {
    key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct AudioSourceRow {
    file_hash: String,
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct DataSaverJobRow {
    id: String,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataSaverJobAction {
    Retry,
    Dispatch,
    Skip,
}
/// Creates and dispatches missing Opus/AAC jobs for every eligible Worker
/// audio source. The idempotency key makes settings saves and cron scans safe
/// to repeat without starting duplicate containers.
pub async fn enqueue_data_saver_jobs(env: &Env) -> Result<(), AppError> {
    let db = env
        .d1("DB")
        .map_err(|error| AppError::Database(error.to_string()))?;
    let (enabled, extensions) = load_data_saver_config(&db).await?;
    if !enabled || extensions.is_empty() {
        return Ok(());
    }

    let sources = db
        .prepare(
            "SELECT file_hash, file_name
             FROM tracks
             WHERE file_hash IS NOT NULL AND file_name IS NOT NULL",
        )
        .all()
        .await
        .map_err(|error| AppError::Database(error.to_string()))?
        .results::<AudioSourceRow>()
        .map_err(|error| AppError::Database(error.to_string()))?;
    if sources.is_empty() {
        return Ok(());
    }

    let config = WasabiConfig::from_env(env)
        .await
        .map_err(|error| AppError::Storage(error.to_string()))?;
    let prefix = config.prefix.clone();
    let storage = WasabiStorage::new(config);
    let repository = D1AudioJobRepository::new(&db);
    let service = AudioJobService::new(repository);
    let mut first_error = None;

    for source in sources {
        let Some(extension) = source_extension(&source.file_name, &source.file_hash) else {
            continue;
        };
        if !extensions.contains(&extension) {
            continue;
        }
        let input_key = match object_key(
            prefix.as_deref(),
            ObjectKind::OriginalAudio,
            &source.file_hash,
            &extension,
        ) {
            Ok(key) => key,
            Err(_) => continue,
        };

        for codec in [AudioCodec::Opus, AudioCodec::Aac] {
            let output_key = match object_key(
                prefix.as_deref(),
                ObjectKind::EncodedAudio { codec },
                &source.file_hash,
                codec.as_str(),
            ) {
                Ok(key) => key,
                Err(_) => continue,
            };
            let idempotency_key = format!("data-saver:{}:{}", source.file_hash, codec.as_str());
            if storage.exists(&output_key).await? {
                continue;
            }
            if let Some(existing) = audio_job_state(&db, &idempotency_key).await? {
                let job_id = match data_saver_job_action(&existing.status) {
                    DataSaverJobAction::Retry => match service.retry(&existing.id).await {
                        Ok(job) => Some(job.id),
                        Err(AppError::Conflict(_)) => None,
                        Err(error) => {
                            first_error.get_or_insert(error);
                            None
                        }
                    },
                    DataSaverJobAction::Dispatch => Some(existing.id),
                    DataSaverJobAction::Skip => None,
                };
                if let Some(job_id) = job_id {
                    if let Err(error) = dispatch(env, &job_id).await {
                        first_error.get_or_insert(error);
                    }
                }
                continue;
            }
            let request = match prepare_request(
                input_key.clone(),
                output_key,
                codec,
                Some(DEFAULT_AUDIO_BITRATE_KBPS),
                Some(idempotency_key),
            ) {
                Ok(request) => request,
                Err(_) => continue,
            };
            match service.submit(&storage, request).await {
                Ok(job) => {
                    if let Err(error) = dispatch(env, &job.id).await {
                        first_error.get_or_insert(error);
                    }
                }
                Err(AppError::NotFound | AppError::Conflict(_)) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
    }

    first_error.map_or(Ok(()), Err)
}

async fn load_data_saver_config(
    db: &worker::D1Database,
) -> Result<(bool, HashSet<String>), AppError> {
    let rows = db
        .prepare(
            "SELECT key, value FROM settings
             WHERE key = 'audio.data_saver.enabled'
                OR key = 'audio.data_saver.extensions'",
        )
        .all()
        .await
        .map_err(|error| AppError::Database(error.to_string()))?
        .results::<SettingRow>()
        .map_err(|error| AppError::Database(error.to_string()))?;
    let mut enabled = false;
    let mut extensions = DATA_SAVER_DEFAULT_EXTENSIONS
        .split(',')
        .filter_map(normalize_extension)
        .collect::<HashSet<_>>();
    for row in rows {
        match row.key.as_str() {
            "audio.data_saver.enabled" => {
                enabled = matches!(row.value.trim(), "true" | "1" | "on");
            }
            "audio.data_saver.extensions" => {
                extensions = row
                    .value
                    .split(',')
                    .filter_map(normalize_extension)
                    .collect();
            }
            _ => {}
        }
    }
    Ok((enabled, extensions))
}

async fn audio_job_state(
    db: &worker::D1Database,
    idempotency_key: &str,
) -> Result<Option<DataSaverJobRow>, AppError> {
    let owner = D1Type::Text(OWNER_SCOPE);
    let idempotency_key = D1Type::Text(idempotency_key);
    db.prepare(
        "SELECT id, status FROM audio_jobs
         WHERE owner = ? AND idempotency_key = ?
         LIMIT 1",
    )
    .bind_refs([&owner, &idempotency_key])
    .map_err(|error| AppError::Database(error.to_string()))?
    .first::<DataSaverJobRow>(None)
    .await
    .map_err(|error| AppError::Database(error.to_string()))
}

fn data_saver_job_action(status: &str) -> DataSaverJobAction {
    match status {
        "failed" => DataSaverJobAction::Retry,
        "queued" => DataSaverJobAction::Dispatch,
        "running" | "completed" => DataSaverJobAction::Skip,
        _ => DataSaverJobAction::Skip,
    }
}
fn source_extension(file_name: &str, file_hash: &str) -> Option<String> {
    file_name
        .rsplit_once('.')
        .and_then(|(_, extension)| normalize_extension(extension))
        .or_else(|| {
            file_hash
                .rsplit_once('.')
                .and_then(|(_, extension)| normalize_extension(extension))
        })
}

fn normalize_extension(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('.').to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 10
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(value)
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

#[cfg(test)]
mod tests {
    use super::{DataSaverJobAction, data_saver_job_action};

    #[test]
    fn data_saver_retries_failed_jobs_and_does_not_duplicate_active_jobs() {
        assert_eq!(data_saver_job_action("failed"), DataSaverJobAction::Retry);
        assert_eq!(
            data_saver_job_action("queued"),
            DataSaverJobAction::Dispatch
        );
        assert_eq!(data_saver_job_action("running"), DataSaverJobAction::Skip);
        assert_eq!(data_saver_job_action("completed"), DataSaverJobAction::Skip);
    }
}
