use std::future::Future;

use dantalian::{
    application::error::AppError,
    ports::{
        audio_jobs::{
            AudioJob, AudioJobClaim, AudioJobFailure, AudioJobRepository, AudioJobRequest,
            AudioJobStatus, MAX_AUDIO_JOB_ATTEMPTS,
        },
        object_storage::AudioCodec,
    },
};
use uuid::Uuid;
use worker::{D1Database, D1Type, Date};

const OWNER_SCOPE: &str = "api-token";
const MIN_LEASE_SECONDS: u64 = 30;
const MAX_LEASE_SECONDS: u64 = 3_600;
const MAX_BACKOFF_SECONDS: u64 = 86_400;

pub(crate) struct D1AudioJobRepository<'a> {
    db: &'a D1Database,
}

impl<'a> D1AudioJobRepository<'a> {
    pub(crate) fn new(db: &'a D1Database) -> Self {
        Self { db }
    }

    async fn load(&self, job_id: &str) -> Result<AudioJob, AppError> {
        self.load_row_by_id(job_id).await?.into_job()
    }

    async fn load_row_by_id(&self, job_id: &str) -> Result<AudioJobRow, AppError> {
        self.db
            .prepare(&format!("{SELECT_COLUMNS} AND id = ?"))
            .bind_refs([&D1Type::Text(OWNER_SCOPE), &D1Type::Text(job_id)])
            .map_err(db_error)?
            .first::<AudioJobRow>(None)
            .await
            .map_err(db_error)?
            .ok_or(AppError::NotFound)
    }

    async fn load_by_idempotency(&self, key: &str) -> Result<Option<AudioJobRow>, AppError> {
        self.db
            .prepare(&format!("{SELECT_COLUMNS} AND idempotency_key = ?"))
            .bind_refs([&D1Type::Text(OWNER_SCOPE), &D1Type::Text(key)])
            .map_err(db_error)?
            .first::<AudioJobRow>(None)
            .await
            .map_err(db_error)
    }

    async fn load_active_output(&self, output_key: &str) -> Result<Option<AudioJobRow>, AppError> {
        self.db
            .prepare(&format!(
                "{SELECT_COLUMNS} AND output_object_key = ?
                     AND status IN ('queued', 'running', 'completed')"
            ))
            .bind_refs([&D1Type::Text(OWNER_SCOPE), &D1Type::Text(output_key)])
            .map_err(db_error)?
            .first::<AudioJobRow>(None)
            .await
            .map_err(db_error)
    }
}

impl AudioJobRepository for D1AudioJobRepository<'_> {
    fn submit(&self, request: AudioJobRequest) -> impl Future<Output = Result<AudioJob, AppError>> {
        async move {
            if let Some(key) = request.idempotency_key.as_deref() {
                if let Some(existing) = self.load_by_idempotency(key).await? {
                    let job = existing.into_job()?;
                    if same_request(&job, &request) {
                        return Ok(job);
                    }
                    return Err(AppError::Conflict(
                        "idempotency key is already used for another audio job".to_string(),
                    ));
                }
            }
            if self
                .load_active_output(&request.output_object_key)
                .await?
                .is_some()
            {
                return Err(AppError::Conflict(
                    "audio job output object is already assigned".to_string(),
                ));
            }

            let id = Uuid::new_v4().simple().to_string();
            let now = now_millis();
            let bitrate_kbps = i32::try_from(request.bitrate_kbps)
                .map_err(|_| AppError::Validation("invalid audio bitrate".to_string()))?;
            let idempotency = request
                .idempotency_key
                .as_deref()
                .map(D1Type::Text)
                .unwrap_or(D1Type::Null);
            self.db
                .prepare(
                    "INSERT INTO audio_jobs
                     (id, status, input_object_key, output_object_key, codec,
                      bitrate_kbps, idempotency_key, attempt_count, error_summary,
                      owner, created_at, updated_at)
                     VALUES (?, 'queued', ?, ?, ?, ?, ?, 0, NULL, ?, ?, ?)",
                )
                .bind_refs([
                    &D1Type::Text(&id),
                    &D1Type::Text(&request.input_object_key),
                    &D1Type::Text(&request.output_object_key),
                    &D1Type::Text(request.codec.as_str()),
                    &D1Type::Integer(bitrate_kbps),
                    &idempotency,
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Text(&now),
                    &D1Type::Text(&now),
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(insert_error)?;
            self.load(&id).await
        }
    }

    fn get(&self, job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>> {
        async move { self.load(job_id).await }
    }

    fn claim_next(
        &self,
        processor_id: &str,
        lease_seconds: u64,
    ) -> impl Future<Output = Result<Option<AudioJobClaim>, AppError>> {
        let processor_id = processor_id.to_string();
        async move {
            validate_processor_id(&processor_id)?;
            let lease_seconds = lease_seconds.clamp(MIN_LEASE_SECONDS, MAX_LEASE_SECONDS);
            let now = Date::now().as_millis();
            let now_value = now.to_string();
            let lease_until = now
                .checked_add(lease_seconds.saturating_mul(1_000))
                .ok_or_else(|| AppError::Validation("audio lease is too long".to_string()))?
                .to_string();
            let lease_token = Uuid::new_v4().simple().to_string();
            let row = self
                .db
                .prepare(
                    r#"UPDATE audio_jobs
                     SET status = 'running',
                         attempt_count = attempt_count + 1,
                         lease_until = ?,
                         started_at = COALESCE(started_at, ?),
                         finished_at = NULL,
                         next_attempt_at = NULL,
                         processor_id = ?,
                         lease_token = ?,
                         updated_at = ?
                     WHERE id = (
                         SELECT id FROM audio_jobs
                         WHERE owner = ?
                           AND attempt_count < ?
                           AND (
                             (status = 'queued' AND
                              (next_attempt_at IS NULL OR CAST(next_attempt_at AS INTEGER) <= ?))
                             OR
                             (status = 'running' AND lease_until IS NOT NULL AND
                              CAST(lease_until AS INTEGER) <= ?)
                           )
                         ORDER BY created_at, id
                         LIMIT 1
                     )
                       AND owner = ?
                       AND attempt_count < ?
                       AND status IN ('queued', 'running')
                     RETURNING id, status, input_object_key, output_object_key, codec,
                        bitrate_kbps, idempotency_key, attempt_count, lease_until,
                        started_at, finished_at, next_attempt_at, processor_id,
                        provider_job_id, output_size_bytes, error_summary,
                        created_at, updated_at, lease_token"#,
                )
                .bind_refs([
                    &D1Type::Text(&lease_until),
                    &D1Type::Text(&now_value),
                    &D1Type::Text(&processor_id),
                    &D1Type::Text(&lease_token),
                    &D1Type::Text(&now_value),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Integer(i32::try_from(MAX_AUDIO_JOB_ATTEMPTS).unwrap_or(i32::MAX)),
                    &D1Type::Text(&now_value),
                    &D1Type::Text(&now_value),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Integer(i32::try_from(MAX_AUDIO_JOB_ATTEMPTS).unwrap_or(i32::MAX)),
                ])
                .map_err(db_error)?
                .first::<AudioJobRow>(None)
                .await
                .map_err(db_error)?;
            row.map(AudioJobRow::into_claim).transpose()
        }
    }

    fn claim_by_id(
        &self,
        job_id: &str,
        processor_id: &str,
        lease_seconds: u64,
    ) -> impl Future<Output = Result<Option<AudioJobClaim>, AppError>> {
        let job_id = job_id.to_string();
        let processor_id = processor_id.to_string();
        async move {
            validate_processor_id(&processor_id)?;
            let lease_seconds = lease_seconds.clamp(MIN_LEASE_SECONDS, MAX_LEASE_SECONDS);
            let now = Date::now().as_millis();
            let now_value = now.to_string();
            let lease_until = now
                .checked_add(lease_seconds.saturating_mul(1_000))
                .ok_or_else(|| AppError::Validation("audio lease is too long".to_string()))?
                .to_string();
            let lease_token = Uuid::new_v4().simple().to_string();
            let row = self
                .db
                .prepare(
                    r#"UPDATE audio_jobs
                     SET status = 'running',
                         attempt_count = attempt_count + 1,
                         lease_until = ?,
                         started_at = COALESCE(started_at, ?),
                         finished_at = NULL,
                         next_attempt_at = NULL,
                         processor_id = ?,
                         lease_token = ?,
                         updated_at = ?
                     WHERE id = ?
                       AND owner = ?
                       AND attempt_count < ?
                       AND (
                         (status = 'queued' AND
                          (next_attempt_at IS NULL OR CAST(next_attempt_at AS INTEGER) <= ?))
                         OR
                         (status = 'running' AND lease_until IS NOT NULL AND
                          CAST(lease_until AS INTEGER) <= ?)
                       )
                     RETURNING id, status, input_object_key, output_object_key, codec,
                        bitrate_kbps, idempotency_key, attempt_count, lease_until,
                        started_at, finished_at, next_attempt_at, processor_id,
                        provider_job_id, output_size_bytes, error_summary,
                        created_at, updated_at, lease_token"#,
                )
                .bind_refs([
                    &D1Type::Text(&lease_until),
                    &D1Type::Text(&now_value),
                    &D1Type::Text(&processor_id),
                    &D1Type::Text(&lease_token),
                    &D1Type::Text(&now_value),
                    &D1Type::Text(&job_id),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Integer(i32::try_from(MAX_AUDIO_JOB_ATTEMPTS).unwrap_or(i32::MAX)),
                    &D1Type::Text(&now_value),
                    &D1Type::Text(&now_value),
                ])
                .map_err(db_error)?
                .first::<AudioJobRow>(None)
                .await
                .map_err(db_error)?;
            row.map(AudioJobRow::into_claim).transpose()
        }
    }

    fn renew_lease(
        &self,
        claim: &AudioJobClaim,
        lease_seconds: u64,
    ) -> impl Future<Output = Result<AudioJobClaim, AppError>> {
        let claim = claim.clone();
        async move {
            let lease_seconds = lease_seconds.clamp(MIN_LEASE_SECONDS, MAX_LEASE_SECONDS);
            let now = Date::now().as_millis();
            let now_value = now.to_string();
            let lease_until = now
                .checked_add(lease_seconds.saturating_mul(1_000))
                .ok_or_else(|| AppError::Validation("audio lease is too long".to_string()))?
                .to_string();
            let row = self
                .db
                .prepare(
                    r#"UPDATE audio_jobs
                     SET lease_until = ?, updated_at = ?
                     WHERE id = ? AND owner = ? AND status = 'running'
                       AND processor_id = ? AND lease_token = ?
                       AND CAST(lease_until AS INTEGER) > ?
                     RETURNING id, status, input_object_key, output_object_key, codec,
                        bitrate_kbps, idempotency_key, attempt_count, lease_until,
                        started_at, finished_at, next_attempt_at, processor_id,
                        provider_job_id, output_size_bytes, error_summary,
                        created_at, updated_at, lease_token"#,
                )
                .bind_refs([
                    &D1Type::Text(&lease_until),
                    &D1Type::Text(&now_value),
                    &D1Type::Text(&claim.job.id),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Text(claim.job.processor_id.as_deref().unwrap_or_default()),
                    &D1Type::Text(&claim.lease_token),
                    &D1Type::Text(&now_value),
                ])
                .map_err(db_error)?
                .first::<AudioJobRow>(None)
                .await
                .map_err(db_error)?
                .ok_or_else(|| {
                    AppError::Conflict("audio job lease is no longer valid".to_string())
                })?;
            row.into_claim()
        }
    }

    fn complete(
        &self,
        claim: &AudioJobClaim,
        output_size_bytes: u64,
    ) -> impl Future<Output = Result<AudioJob, AppError>> {
        let claim = claim.clone();
        async move {
            let now = now_millis();
            let output_size = i32::try_from(output_size_bytes)
                .map_err(|_| AppError::Validation("audio output is too large".to_string()))?;
            let result = self
                .db
                .prepare(
                    "UPDATE audio_jobs
                     SET status = 'completed', finished_at = ?, updated_at = ?,
                         lease_until = NULL, processor_id = NULL, lease_token = NULL,
                         next_attempt_at = NULL, output_size_bytes = ?, error_summary = NULL
                     WHERE id = ? AND owner = ? AND status = 'running'
                       AND processor_id = ? AND lease_token = ?",
                )
                .bind_refs([
                    &D1Type::Text(&now),
                    &D1Type::Text(&now),
                    &D1Type::Integer(output_size),
                    &D1Type::Text(&claim.job.id),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Text(claim.job.processor_id.as_deref().unwrap_or_default()),
                    &D1Type::Text(&claim.lease_token),
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
            ensure_changed(result, "audio job lease is no longer valid")?;
            self.load(&claim.job.id).await
        }
    }

    fn fail(
        &self,
        claim: &AudioJobClaim,
        failure: AudioJobFailure,
    ) -> impl Future<Output = Result<AudioJob, AppError>> {
        let claim = claim.clone();
        async move {
            let error_summary = validate_error_summary(&failure.error_summary)?.to_string();
            let now = Date::now().as_millis();
            let now_value = now.to_string();
            let can_retry = failure.retryable && claim.job.attempt_count < MAX_AUDIO_JOB_ATTEMPTS;
            let backoff = failure.backoff_seconds.min(MAX_BACKOFF_SECONDS);
            let next_attempt = now
                .checked_add(backoff.saturating_mul(1_000))
                .ok_or_else(|| AppError::Validation("audio retry delay is too long".to_string()))?
                .to_string();
            let status = if can_retry { "queued" } else { "failed" };
            let finished_at = if can_retry {
                D1Type::Null
            } else {
                D1Type::Text(&now_value)
            };
            let next_attempt_at = if can_retry {
                D1Type::Text(&next_attempt)
            } else {
                D1Type::Null
            };
            let result = self
                .db
                .prepare(
                    "UPDATE audio_jobs
                     SET status = ?, error_summary = ?, finished_at = ?,
                         next_attempt_at = ?, lease_until = NULL, processor_id = NULL,
                         lease_token = NULL, updated_at = ?
                     WHERE id = ? AND owner = ? AND status = 'running'
                       AND processor_id = ? AND lease_token = ?",
                )
                .bind_refs([
                    &D1Type::Text(status),
                    &D1Type::Text(&error_summary),
                    &finished_at,
                    &next_attempt_at,
                    &D1Type::Text(&now_value),
                    &D1Type::Text(&claim.job.id),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Text(claim.job.processor_id.as_deref().unwrap_or_default()),
                    &D1Type::Text(&claim.lease_token),
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
            ensure_changed(result, "audio job lease is no longer valid")?;
            self.load(&claim.job.id).await
        }
    }

    fn retry(&self, job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>> {
        async move {
            let now = now_millis();
            let result = self
                .db
                .prepare(
                    "UPDATE audio_jobs
                     SET status = 'queued', attempt_count = 0, error_summary = NULL,
                         lease_until = NULL, started_at = NULL, finished_at = NULL,
                         next_attempt_at = NULL, processor_id = NULL, lease_token = NULL,
                         output_size_bytes = NULL, updated_at = ?
                     WHERE id = ? AND owner = ? AND status = 'failed'",
                )
                .bind_refs([
                    &D1Type::Text(&now),
                    &D1Type::Text(job_id),
                    &D1Type::Text(OWNER_SCOPE),
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
            if result
                .meta()
                .map_err(db_error)?
                .and_then(|meta| meta.changes)
                .unwrap_or_default()
                == 0
            {
                let job = self.load(job_id).await?;
                return Err(AppError::Conflict(format!(
                    "audio job cannot retry from {}",
                    job.status.as_str()
                )));
            }
            self.load(job_id).await
        }
    }

    fn recover_expired(&self) -> impl Future<Output = Result<u32, AppError>> {
        async move {
            let now = now_millis();
            let requeued = self
                .db
                .prepare(
                    "UPDATE audio_jobs
                     SET status = 'queued',
                         error_summary = 'audio job lease expired; requeued',
                         finished_at = NULL, updated_at = ?, lease_until = NULL,
                         processor_id = NULL, lease_token = NULL, next_attempt_at = NULL
                     WHERE owner = ? AND status = 'running'
                       AND attempt_count < ? AND lease_until IS NOT NULL
                       AND CAST(lease_until AS INTEGER) <= ?",
                )
                .bind_refs([
                    &D1Type::Text(&now),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Integer(i32::try_from(MAX_AUDIO_JOB_ATTEMPTS).unwrap_or(i32::MAX)),
                    &D1Type::Text(&now),
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
            let failed = self
                .db
                .prepare(
                    "UPDATE audio_jobs
                     SET status = 'failed',
                         error_summary = 'audio job lease expired after max attempts',
                         finished_at = ?, updated_at = ?, lease_until = NULL,
                         processor_id = NULL, lease_token = NULL, next_attempt_at = NULL
                     WHERE owner = ? AND status = 'running'
                       AND attempt_count >= ? AND lease_until IS NOT NULL
                       AND CAST(lease_until AS INTEGER) <= ?",
                )
                .bind_refs([
                    &D1Type::Text(&now),
                    &D1Type::Text(&now),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Integer(i32::try_from(MAX_AUDIO_JOB_ATTEMPTS).unwrap_or(i32::MAX)),
                    &D1Type::Text(&now),
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
            let changed = |result: worker::d1::D1Result| -> Result<usize, AppError> {
                Ok(result
                    .meta()
                    .map_err(db_error)?
                    .and_then(|meta| meta.changes)
                    .unwrap_or_default())
            };
            Ok((changed(requeued)? + changed(failed)?) as u32)
        }
    }

    fn dispatchable_ids(&self, limit: u32) -> impl Future<Output = Result<Vec<String>, AppError>> {
        let limit = i32::try_from(limit.clamp(1, 100)).unwrap_or(100);
        async move {
            let now = now_millis();
            let rows = self
                .db
                .prepare(
                    "SELECT id FROM audio_jobs
                     WHERE owner = ? AND status = 'queued'
                       AND attempt_count < ?
                       AND (next_attempt_at IS NULL OR CAST(next_attempt_at AS INTEGER) <= ?)
                     ORDER BY created_at, id
                     LIMIT ?",
                )
                .bind_refs([
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Integer(i32::try_from(MAX_AUDIO_JOB_ATTEMPTS).unwrap_or(i32::MAX)),
                    &D1Type::Text(&now),
                    &D1Type::Integer(limit),
                ])
                .map_err(db_error)?
                .all()
                .await
                .map_err(db_error)?
                .results::<AudioJobIdRow>()
                .map_err(db_error)?;
            Ok(rows.into_iter().map(|row| row.id).collect())
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct AudioJobIdRow {
    id: String,
}

#[derive(Debug, serde::Deserialize)]
struct AudioJobRow {
    id: String,
    status: String,
    input_object_key: String,
    output_object_key: String,
    codec: String,
    bitrate_kbps: i64,
    idempotency_key: Option<String>,
    attempt_count: i64,
    lease_until: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    next_attempt_at: Option<String>,
    processor_id: Option<String>,
    provider_job_id: Option<String>,
    output_size_bytes: Option<i64>,
    error_summary: Option<String>,
    created_at: String,
    updated_at: String,
    lease_token: Option<String>,
}

impl AudioJobRow {
    fn into_job(self) -> Result<AudioJob, AppError> {
        let codec = match self.codec.as_str() {
            "opus" => AudioCodec::Opus,
            "aac" => AudioCodec::Aac,
            _ => return Err(AppError::Database("invalid audio job codec".to_string())),
        };
        let bitrate_kbps = u32::try_from(self.bitrate_kbps)
            .map_err(|_| AppError::Database("invalid audio job bitrate".to_string()))?;
        let attempt_count = u32::try_from(self.attempt_count)
            .map_err(|_| AppError::Database("invalid audio job attempt count".to_string()))?;
        let output_size_bytes = self
            .output_size_bytes
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| AppError::Database("invalid audio output size".to_string()))
            })
            .transpose()?;
        Ok(AudioJob {
            id: self.id,
            status: AudioJobStatus::parse(&self.status)?,
            input_object_key: self.input_object_key,
            output_object_key: self.output_object_key,
            codec,
            bitrate_kbps,
            idempotency_key: self.idempotency_key,
            attempt_count,
            lease_until: self.lease_until,
            started_at: self.started_at,
            finished_at: self.finished_at,
            next_attempt_at: self.next_attempt_at,
            processor_id: self.processor_id,
            provider_job_id: self.provider_job_id,
            output_size_bytes,
            error_summary: self.error_summary,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }

    fn into_claim(self) -> Result<AudioJobClaim, AppError> {
        let lease_token = self
            .lease_token
            .clone()
            .ok_or_else(|| AppError::Database("audio job lease token is missing".to_string()))?;
        Ok(AudioJobClaim {
            job: self.into_job()?,
            lease_token,
        })
    }
}

const SELECT_COLUMNS: &str = "SELECT id, status, input_object_key, output_object_key, codec,
    bitrate_kbps, idempotency_key, attempt_count, lease_until, started_at,
    finished_at, next_attempt_at, processor_id, provider_job_id, output_size_bytes,
    error_summary, created_at, updated_at, lease_token
    FROM audio_jobs WHERE owner = ?";

fn same_request(job: &AudioJob, request: &AudioJobRequest) -> bool {
    job.input_object_key == request.input_object_key
        && job.output_object_key == request.output_object_key
        && job.codec == request.codec
        && job.bitrate_kbps == request.bitrate_kbps
}

fn validate_processor_id(value: &str) -> Result<(), AppError> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "processor id must be 1-128 printable characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_error_summary(value: &str) -> Result<&str, AppError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 1_000 || value.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "audio job error summary must be 1-1000 printable characters".to_string(),
        ));
    }
    Ok(value)
}

fn now_millis() -> String {
    Date::now().as_millis().to_string()
}

fn ensure_changed(meta: worker::d1::D1Result, message: &str) -> Result<(), AppError> {
    let changed = meta
        .meta()
        .map_err(db_error)?
        .and_then(|value| value.changes)
        .unwrap_or_default();
    if changed == 0 {
        Err(AppError::Conflict(message.to_string()))
    } else {
        Ok(())
    }
}

fn db_error(error: impl std::fmt::Display) -> AppError {
    AppError::Database(error.to_string())
}

fn insert_error(error: impl std::fmt::Display) -> AppError {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("unique") {
        AppError::Conflict("audio job already exists".to_string())
    } else {
        AppError::Database(message)
    }
}
