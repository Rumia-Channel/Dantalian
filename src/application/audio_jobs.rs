use crate::{
    application::error::AppError,
    ports::{
        audio_jobs::{AudioJob, AudioJobQueue, AudioJobRequest},
        object_storage::{AudioCodec, ObjectStorage, validate_object_key},
    },
};

pub const DEFAULT_AUDIO_BITRATE_KBPS: u32 = 192;

pub struct AudioJobService<Q> {
    queue: Q,
}

impl<Q> AudioJobService<Q> {
    pub fn new(queue: Q) -> Self {
        Self { queue }
    }
}

impl<Q> AudioJobService<Q>
where
    Q: AudioJobQueue,
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
        self.queue.submit(request).await
    }

    pub async fn get(&self, job_id: &str) -> Result<AudioJob, AppError> {
        self.queue.get(job_id).await
    }
}

pub fn prepare_request(
    input_object_key: String,
    output_object_key: String,
    codec: AudioCodec,
    bitrate_kbps: Option<u32>,
) -> Result<AudioJobRequest, AppError> {
    let request = AudioJobRequest {
        input_object_key,
        output_object_key,
        codec,
        bitrate_kbps: bitrate_kbps.unwrap_or(DEFAULT_AUDIO_BITRATE_KBPS),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct FakeStorage {
        input_exists: bool,
        output_exists: bool,
    }

    impl ObjectStorage for FakeStorage {
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

    struct FakeQueue;

    impl AudioJobQueue for FakeQueue {
        fn submit(
            &self,
            request: AudioJobRequest,
        ) -> impl Future<Output = Result<AudioJob, AppError>> {
            async move {
                Ok(AudioJob {
                    id: "job-id".to_string(),
                    status: crate::ports::audio_jobs::AudioJobStatus::Queued,
                    input_object_key: request.input_object_key,
                    output_object_key: request.output_object_key,
                    codec: request.codec,
                    bitrate_kbps: request.bitrate_kbps,
                    error_summary: None,
                    created_at: "now".to_string(),
                    updated_at: "now".to_string(),
                })
            }
        }

        fn get(&self, _job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Err(AppError::Internal("unused fake queue method".to_string())) }
        }

        fn mark_running(&self, _job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Err(AppError::Internal("unused fake queue method".to_string())) }
        }

        fn mark_completed(
            &self,
            _job_id: &str,
        ) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Err(AppError::Internal("unused fake queue method".to_string())) }
        }

        fn mark_failed(
            &self,
            _job_id: &str,
            _error_summary: &str,
        ) -> impl Future<Output = Result<AudioJob, AppError>> {
            async { Err(AppError::Internal("unused fake queue method".to_string())) }
        }
    }

    fn request(bitrate_kbps: Option<u32>) -> AudioJobRequest {
        prepare_request(
            "audio/input.mp3".to_string(),
            "audio/output.opus".to_string(),
            AudioCodec::Opus,
            bitrate_kbps,
        )
        .expect("valid audio job request")
    }

    #[test]
    fn prepares_default_and_validates_audio_parameters() {
        assert_eq!(request(None).bitrate_kbps, DEFAULT_AUDIO_BITRATE_KBPS);
        assert!(
            prepare_request(
                "audio/input.mp3".to_string(),
                "audio/output.aac".to_string(),
                AudioCodec::Opus,
                Some(192),
            )
            .is_err()
        );
        assert!(
            prepare_request(
                "audio/input.mp3".to_string(),
                "audio/output.opus".to_string(),
                AudioCodec::Opus,
                Some(7),
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn submit_checks_input_and_output_objects_before_queueing() {
        let service = AudioJobService::new(FakeQueue);

        let missing_storage = FakeStorage {
            input_exists: false,
            output_exists: false,
        };
        let missing_input = service.submit(&missing_storage, request(None)).await;
        assert!(matches!(missing_input, Err(AppError::NotFound)));

        let occupied_storage = FakeStorage {
            input_exists: true,
            output_exists: true,
        };
        let occupied_output = service.submit(&occupied_storage, request(None)).await;
        assert!(matches!(occupied_output, Err(AppError::Conflict(_))));

        let available_storage = FakeStorage {
            input_exists: true,
            output_exists: false,
        };
        let queued = service
            .submit(&available_storage, request(None))
            .await
            .expect("job should be queued");
        assert_eq!(
            queued.status,
            crate::ports::audio_jobs::AudioJobStatus::Queued
        );
    }
}
