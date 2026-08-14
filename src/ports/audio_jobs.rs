use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::application::error::AppError;
use crate::ports::object_storage::AudioCodec;

pub const MAX_AUDIO_JOB_ATTEMPTS: u32 = 3;
pub const DEFAULT_AUDIO_JOB_LEASE_SECONDS: u64 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl AudioJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(AppError::Database("invalid audio job status".to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioJobRequest {
    pub input_object_key: String,
    pub output_object_key: String,
    pub codec: AudioCodec,
    pub bitrate_kbps: u32,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioJob {
    pub id: String,
    pub status: AudioJobStatus,
    pub input_object_key: String,
    pub output_object_key: String,
    pub codec: AudioCodec,
    pub bitrate_kbps: u32,
    pub idempotency_key: Option<String>,
    pub attempt_count: u32,
    pub lease_until: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub next_attempt_at: Option<String>,
    pub processor_id: Option<String>,
    pub provider_job_id: Option<String>,
    pub output_size_bytes: Option<u64>,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioJobClaim {
    pub job: AudioJob,
    pub lease_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioJobFailure {
    pub error_summary: String,
    pub retryable: bool,
    pub backoff_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioJobDispatchMessage {
    pub version: u8,
    pub job_id: String,
}

pub trait AudioJobRepository {
    fn submit(&self, request: AudioJobRequest) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn get(&self, job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn claim_next(
        &self,
        processor_id: &str,
        lease_seconds: u64,
    ) -> impl Future<Output = Result<Option<AudioJobClaim>, AppError>>;
    fn claim_by_id(
        &self,
        job_id: &str,
        processor_id: &str,
        lease_seconds: u64,
    ) -> impl Future<Output = Result<Option<AudioJobClaim>, AppError>>;
    fn renew_lease(
        &self,
        claim: &AudioJobClaim,
        lease_seconds: u64,
    ) -> impl Future<Output = Result<AudioJobClaim, AppError>>;
    fn complete(
        &self,
        claim: &AudioJobClaim,
        output_size_bytes: u64,
    ) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn fail(
        &self,
        claim: &AudioJobClaim,
        failure: AudioJobFailure,
    ) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn retry(&self, job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn recover_expired(&self) -> impl Future<Output = Result<u32, AppError>>;
    fn dispatchable_ids(&self, limit: u32) -> impl Future<Output = Result<Vec<String>, AppError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_serializes_statuses() {
        assert_eq!(
            AudioJobStatus::parse("queued").unwrap(),
            AudioJobStatus::Queued
        );
        assert_eq!(AudioJobStatus::Completed.as_str(), "completed");
        assert!(AudioJobStatus::parse("waiting").is_err());
        assert_eq!(
            serde_json::to_string(&AudioJobStatus::Running).unwrap(),
            "\"running\""
        );
    }
}
