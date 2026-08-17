use crate::{
    application::error::AppError,
    ports::{
        audio_jobs::{
            AudioJob, AudioJobClaim, AudioJobFailure, AudioJobRepository, AudioJobRequest,
        },
        object_storage::{AudioCodec, ObjectStorage, validate_object_key},
    },
};

pub const DEFAULT_AUDIO_BITRATE_KBPS: u32 = 192;

pub struct AudioJobService<Q> {
    repository: Q,
}

impl<Q> AudioJobService<Q> {
    pub fn new(repository: Q) -> Self {
        Self { repository }
    }
}

impl<Q> AudioJobService<Q>
where
    Q: AudioJobRepository,
{
    pub async fn submit<S: ObjectStorage>(
        &self,
        storage: &S,
        request: AudioJobRequest,
    ) -> Result<AudioJob, AppError> {
        let request = validate_request(request)?;
        if !storage.exists(&request.input_object_key).await? {
            return Err(AppError::NotFound);
        }
        if storage.exists(&request.output_object_key).await? {
            return Err(AppError::Conflict(
                "audio job output object already exists".to_string(),
            ));
        }
        self.repository.submit(request).await
    }

    pub async fn get(&self, job_id: &str) -> Result<AudioJob, AppError> {
        self.repository.get(job_id).await
    }

    pub async fn claim_next(
        &self,
        processor_id: &str,
        lease_seconds: u64,
    ) -> Result<Option<AudioJobClaim>, AppError> {
        self.repository
            .claim_next(processor_id, lease_seconds)
            .await
    }

    pub async fn claim_by_id(
        &self,
        job_id: &str,
        processor_id: &str,
        lease_seconds: u64,
    ) -> Result<Option<AudioJobClaim>, AppError> {
        self.repository
            .claim_by_id(job_id, processor_id, lease_seconds)
            .await
    }

    pub async fn renew_lease(
        &self,
        claim: &AudioJobClaim,
        lease_seconds: u64,
    ) -> Result<AudioJobClaim, AppError> {
        self.repository.renew_lease(claim, lease_seconds).await
    }

    pub async fn complete<S: ObjectStorage>(
        &self,
        storage: &S,
        claim: &AudioJobClaim,
    ) -> Result<AudioJob, AppError> {
        let metadata = storage.head(&claim.job.output_object_key).await?;
        let output_size_bytes = metadata.content_length.unwrap_or_default();
        if output_size_bytes == 0 {
            return Err(AppError::Conflict(
                "audio job output object is empty".to_string(),
            ));
        }
        self.repository.complete(claim, output_size_bytes).await
    }

    pub async fn fail(
        &self,
        claim: &AudioJobClaim,
        failure: AudioJobFailure,
    ) -> Result<AudioJob, AppError> {
        self.repository.fail(claim, failure).await
    }

    pub async fn retry(&self, job_id: &str) -> Result<AudioJob, AppError> {
        self.repository.retry(job_id).await
    }

    pub async fn requeue_missing_output<S: ObjectStorage>(
        &self,
        storage: &S,
        job_id: &str,
    ) -> Result<AudioJob, AppError> {
        let job = self.repository.get(job_id).await?;
        if storage.exists(&job.output_object_key).await? {
            return Err(AppError::Conflict(
                "audio job output object still exists".to_string(),
            ));
        }
        self.repository.requeue_missing_output(job_id).await
    }

    pub async fn recover_expired(&self) -> Result<u32, AppError> {
        self.repository.recover_expired().await
    }

    pub async fn dispatchable_ids(&self, limit: u32) -> Result<Vec<String>, AppError> {
        self.repository.dispatchable_ids(limit).await
    }
}

pub fn prepare_request(
    input_object_key: String,
    output_object_key: String,
    codec: AudioCodec,
    bitrate_kbps: Option<u32>,
    idempotency_key: Option<String>,
) -> Result<AudioJobRequest, AppError> {
    let request = AudioJobRequest {
        input_object_key,
        output_object_key,
        codec,
        bitrate_kbps: bitrate_kbps.unwrap_or(DEFAULT_AUDIO_BITRATE_KBPS),
        idempotency_key: normalize_idempotency_key(idempotency_key)?,
    };
    validate_request(request)
}

fn validate_request(request: AudioJobRequest) -> Result<AudioJobRequest, AppError> {
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
    Ok(request)
}

fn normalize_idempotency_key(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "idempotency key must be 1-128 printable characters".to_string(),
        ));
    }
    Ok(Some(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::audio_jobs::{AudioJobStatus, MAX_AUDIO_JOB_ATTEMPTS};
    use crate::ports::object_storage::ObjectMetadata;
    use std::future::Future;

    #[derive(Clone, Copy)]
    struct FakeStorage {
        input_exists: bool,
        output_exists: bool,
        output_size: Option<u64>,
    }

    impl ObjectStorage for FakeStorage {
        fn head(&self, key: &str) -> impl Future<Output = Result<ObjectMetadata, AppError>> {
            let result = if key == "audio/output.opus" {
                match self.output_size {
                    Some(size) => Ok(ObjectMetadata {
                        content_length: Some(size),
                        content_type: Some("audio/ogg".to_string()),
                    }),
                    None => Err(AppError::NotFound),
                }
            } else if key == "audio/input.mp3" && self.input_exists {
                Ok(ObjectMetadata {
                    content_length: Some(10),
                    content_type: Some("audio/mpeg".to_string()),
                })
            } else {
                Err(AppError::NotFound)
            };
            async move { result }
        }

        fn exists(&self, key: &str) -> impl Future<Output = Result<bool, AppError>> {
            let exists = match key {
                "audio/input.mp3" => self.input_exists,
                "audio/output.opus" => self.output_exists,
                _ => false,
            };
            async move { Ok(exists) }
        }

        fn put_object(
            &self,
            _key: &str,
            _content_type: &str,
            _bytes: &[u8],
        ) -> impl Future<Output = Result<(), AppError>> {
            async { Err(AppError::Internal("unused fake storage method".to_string())) }
        }

        fn delete(&self, _key: &str) -> impl Future<Output = Result<(), AppError>> {
            async { Err(AppError::Internal("unused fake storage method".to_string())) }
        }

        fn temporary_get_url(&self, _key: &str) -> impl Future<Output = Result<String, AppError>> {
            async { Err(AppError::Internal("unused fake storage method".to_string())) }
        }
    }

    struct FakeRepository;

    fn job(status: AudioJobStatus) -> AudioJob {
        AudioJob {
            id: "job-id".to_string(),
            status,
            input_object_key: "audio/input.mp3".to_string(),
            output_object_key: "audio/output.opus".to_string(),
            codec: AudioCodec::Opus,
            bitrate_kbps: DEFAULT_AUDIO_BITRATE_KBPS,
            idempotency_key: None,
            attempt_count: 1,
            lease_until: None,
            started_at: None,
            finished_at: None,
            next_attempt_at: None,
            processor_id: None,
            provider_job_id: None,
            output_size_bytes: None,
            error_summary: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    impl AudioJobRepository for FakeRepository {
        fn submit(
            &self,
            _request: AudioJobRequest,
        ) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Ok(job(AudioJobStatus::Queued)) }
        }

        fn get(&self, _job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Ok(job(AudioJobStatus::Queued)) }
        }

        fn claim_next(
            &self,
            _processor_id: &str,
            _lease_seconds: u64,
        ) -> impl Future<Output = Result<Option<AudioJobClaim>, AppError>> {
            async {
                Ok(Some(AudioJobClaim {
                    job: job(AudioJobStatus::Running),
                    lease_token: "lease-token".to_string(),
                }))
            }
        }

        fn claim_by_id(
            &self,
            _job_id: &str,
            _processor_id: &str,
            _lease_seconds: u64,
        ) -> impl Future<Output = Result<Option<AudioJobClaim>, AppError>> {
            async {
                Ok(Some(AudioJobClaim {
                    job: job(AudioJobStatus::Running),
                    lease_token: "lease-token".to_string(),
                }))
            }
        }

        fn renew_lease(
            &self,
            claim: &AudioJobClaim,
            _lease_seconds: u64,
        ) -> impl Future<Output = Result<AudioJobClaim, AppError>> {
            let claim = claim.clone();
            async move { Ok(claim) }
        }

        fn complete(
            &self,
            _claim: &AudioJobClaim,
            _output_size_bytes: u64,
        ) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Ok(job(AudioJobStatus::Completed)) }
        }

        fn fail(
            &self,
            _claim: &AudioJobClaim,
            _failure: AudioJobFailure,
        ) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Ok(job(AudioJobStatus::Failed)) }
        }

        fn retry(&self, _job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Ok(job(AudioJobStatus::Queued)) }
        }
        fn requeue_missing_output(
            &self,
            _job_id: &str,
        ) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Ok(job(AudioJobStatus::Queued)) }
        }

        fn recover_expired(&self) -> impl Future<Output = Result<u32, AppError>> {
            async { Ok(0) }
        }

        fn dispatchable_ids(
            &self,
            _limit: u32,
        ) -> impl Future<Output = Result<Vec<String>, AppError>> {
            async { Ok(Vec::new()) }
        }
    }

    fn request(bitrate_kbps: Option<u32>) -> AudioJobRequest {
        prepare_request(
            "audio/input.mp3".to_string(),
            "audio/output.opus".to_string(),
            AudioCodec::Opus,
            bitrate_kbps,
            None,
        )
        .expect("valid audio job request")
    }

    fn claim() -> AudioJobClaim {
        AudioJobClaim {
            job: job(AudioJobStatus::Running),
            lease_token: "lease-token".to_string(),
        }
    }

    #[test]
    fn prepares_default_and_validates_audio_parameters() {
        assert_eq!(request(None).bitrate_kbps, DEFAULT_AUDIO_BITRATE_KBPS);
        assert_eq!(
            prepare_request(
                "audio/input.mp3".to_string(),
                "audio/output.opus".to_string(),
                AudioCodec::Opus,
                Some(192),
                Some(" request-1 ".to_string()),
            )
            .unwrap()
            .idempotency_key,
            Some("request-1".to_string())
        );
        assert!(
            prepare_request(
                "audio/input.mp3".to_string(),
                "audio/output.aac".to_string(),
                AudioCodec::Opus,
                Some(192),
                None,
            )
            .is_err()
        );
        assert!(
            prepare_request(
                "audio/input.mp3".to_string(),
                "audio/output.opus".to_string(),
                AudioCodec::Opus,
                Some(7),
                None,
            )
            .is_err()
        );
        assert!(
            prepare_request(
                "audio/input.mp3".to_string(),
                "audio/output.opus".to_string(),
                AudioCodec::Opus,
                None,
                Some("\n".to_string()),
            )
            .is_err()
        );
        assert!(MAX_AUDIO_JOB_ATTEMPTS >= 1);
    }

    #[tokio::test]
    async fn submit_checks_input_and_output_objects_before_queueing() {
        let service = AudioJobService::new(FakeRepository);
        let missing_storage = FakeStorage {
            input_exists: false,
            output_exists: false,
            output_size: None,
        };
        assert!(matches!(
            service.submit(&missing_storage, request(None)).await,
            Err(AppError::NotFound)
        ));

        let occupied_storage = FakeStorage {
            input_exists: true,
            output_exists: true,
            output_size: Some(10),
        };
        assert!(matches!(
            service.submit(&occupied_storage, request(None)).await,
            Err(AppError::Conflict(_))
        ));
    }

    #[tokio::test]
    async fn completion_requires_non_empty_output_object() {
        let service = AudioJobService::new(FakeRepository);
        let missing = FakeStorage {
            input_exists: true,
            output_exists: false,
            output_size: None,
        };
        assert!(matches!(
            service.complete(&missing, &claim()).await,
            Err(AppError::NotFound)
        ));

        let empty = FakeStorage {
            input_exists: true,
            output_exists: true,
            output_size: Some(0),
        };
        assert!(matches!(
            service.complete(&empty, &claim()).await,
            Err(AppError::Conflict(_))
        ));

        let valid = FakeStorage {
            input_exists: true,
            output_exists: true,
            output_size: Some(42),
        };
        assert_eq!(
            service.complete(&valid, &claim()).await.unwrap().status,
            AudioJobStatus::Completed
        );
    }
}
