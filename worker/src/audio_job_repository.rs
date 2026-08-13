use dantalian::{
    application::error::AppError,
    ports::{
        audio_jobs::{AudioJob, AudioJobQueue, AudioJobRequest, AudioJobStatus},
        object_storage::{AudioCodec, validate_object_key},
    },
};
use uuid::Uuid;
use worker::{D1Database, D1Type, Date};

const OWNER_SCOPE: &str = "api-token";

pub(crate) struct D1AudioJobQueue<'a> {
    db: &'a D1Database,
}

impl<'a> D1AudioJobQueue<'a> {
    pub(crate) fn new(db: &'a D1Database) -> Self {
        Self { db }
    }

    async fn load(&self, job_id: &str) -> Result<AudioJob, AppError> {
        let row = self
            .db
            .prepare(
                "SELECT id, status, input_object_key, output_object_key, codec,
                        bitrate_kbps, error_summary, created_at, updated_at
                 FROM audio_jobs WHERE id = ? AND owner = ?",
            )
            .bind_refs([&D1Type::Text(job_id), &D1Type::Text(OWNER_SCOPE)])
            .map_err(db_error)?
            .first::<AudioJobRow>(None)
            .await
            .map_err(db_error)?
            .ok_or(AppError::NotFound)?;
        row.into_job()
    }

    async fn transition(
        &self,
        job_id: &str,
        from_status: &str,
        status: &str,
        error_summary: Option<&str>,
    ) -> Result<AudioJob, AppError> {
        let error_summary = match error_summary {
            Some(value) => Some(validate_error_summary(value)?.to_string()),
            None => None,
        };
        let error_value = error_summary
            .as_deref()
            .map(D1Type::Text)
            .unwrap_or(D1Type::Null);
        let now = Date::now().as_millis().to_string();
        let result = self
            .db
            .prepare(
                "UPDATE audio_jobs
                 SET status = ?, error_summary = ?, updated_at = ?
                 WHERE id = ? AND owner = ? AND status = ?",
            )
            .bind_refs([
                &D1Type::Text(status),
                &error_value,
                &D1Type::Text(&now),
                &D1Type::Text(job_id),
                &D1Type::Text(OWNER_SCOPE),
                &D1Type::Text(from_status),
            ])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
        let changed = result
            .meta()
            .map_err(db_error)?
            .and_then(|meta| meta.changes)
            .unwrap_or_default();
        if changed == 0 {
            let job = self.load(job_id).await?;
            return Err(AppError::Conflict(format!(
                "audio job cannot transition from {}",
                job.status.as_str()
            )));
        }
        self.load(job_id).await
    }

    async fn fail(&self, job_id: &str, error_summary: &str) -> Result<AudioJob, AppError> {
        let error_summary = validate_error_summary(error_summary)?.to_string();
        let now = Date::now().as_millis().to_string();
        let result = self
            .db
            .prepare(
                "UPDATE audio_jobs
                 SET status = 'failed', error_summary = ?, updated_at = ?
                 WHERE id = ? AND owner = ? AND status IN ('queued', 'running')",
            )
            .bind_refs([
                &D1Type::Text(&error_summary),
                &D1Type::Text(&now),
                &D1Type::Text(job_id),
                &D1Type::Text(OWNER_SCOPE),
            ])
            .map_err(db_error)?
            .run()
            .await
            .map_err(db_error)?;
        let changed = result
            .meta()
            .map_err(db_error)?
            .and_then(|meta| meta.changes)
            .unwrap_or_default();
        if changed == 0 {
            let job = self.load(job_id).await?;
            return Err(AppError::Conflict(format!(
                "audio job cannot fail from {}",
                job.status.as_str()
            )));
        }
        self.load(job_id).await
    }
}

impl AudioJobQueue for D1AudioJobQueue<'_> {
    fn submit(
        &self,
        request: AudioJobRequest,
    ) -> impl std::future::Future<Output = Result<AudioJob, AppError>> {
        async move {
            validate_request(&request)?;
            let id = Uuid::new_v4().simple().to_string();
            let now = Date::now().as_millis().to_string();
            let bitrate_kbps = i32::try_from(request.bitrate_kbps)
                .map_err(|_| AppError::Validation("invalid audio bitrate".to_string()))?;
            let codec = request.codec.as_str();
            self.db
                .prepare(
                    "INSERT INTO audio_jobs
                     (id, status, input_object_key, output_object_key, codec,
                      bitrate_kbps, error_summary, owner, created_at, updated_at)
                     VALUES (?, 'queued', ?, ?, ?, ?, NULL, ?, ?, ?)",
                )
                .bind_refs([
                    &D1Type::Text(&id),
                    &D1Type::Text(&request.input_object_key),
                    &D1Type::Text(&request.output_object_key),
                    &D1Type::Text(codec),
                    &D1Type::Integer(bitrate_kbps),
                    &D1Type::Text(OWNER_SCOPE),
                    &D1Type::Text(&now),
                    &D1Type::Text(&now),
                ])
                .map_err(db_error)?
                .run()
                .await
                .map_err(db_error)?;
            self.load(&id).await
        }
    }

    fn get(&self, job_id: &str) -> impl std::future::Future<Output = Result<AudioJob, AppError>> {
        async move { self.load(job_id).await }
    }

    fn mark_running(
        &self,
        job_id: &str,
    ) -> impl std::future::Future<Output = Result<AudioJob, AppError>> {
        async move { self.transition(job_id, "queued", "running", None).await }
    }

    fn mark_completed(
        &self,
        job_id: &str,
    ) -> impl std::future::Future<Output = Result<AudioJob, AppError>> {
        async move { self.transition(job_id, "running", "completed", None).await }
    }

    fn mark_failed(
        &self,
        job_id: &str,
        error_summary: &str,
    ) -> impl std::future::Future<Output = Result<AudioJob, AppError>> {
        async move { self.fail(job_id, error_summary).await }
    }
}

#[derive(Debug, serde::Deserialize)]
struct AudioJobRow {
    id: String,
    status: String,
    input_object_key: String,
    output_object_key: String,
    codec: String,
    bitrate_kbps: i64,
    error_summary: Option<String>,
    created_at: String,
    updated_at: String,
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
        Ok(AudioJob {
            id: self.id,
            status: AudioJobStatus::parse(&self.status)?,
            input_object_key: self.input_object_key,
            output_object_key: self.output_object_key,
            codec,
            bitrate_kbps,
            error_summary: self.error_summary,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

fn validate_request(request: &AudioJobRequest) -> Result<(), AppError> {
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
    if !(8..=512).contains(&request.bitrate_kbps) {
        return Err(AppError::Validation(
            "audio bitrate must be between 8 and 512 kbps".to_string(),
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

fn db_error(error: impl std::fmt::Display) -> AppError {
    AppError::Database(error.to_string())
}
