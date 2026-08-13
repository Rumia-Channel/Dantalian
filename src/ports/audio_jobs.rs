use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::application::error::AppError;
use crate::ports::object_storage::AudioCodec;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioJob {
    pub id: String,
    pub status: AudioJobStatus,
    pub input_object_key: String,
    pub output_object_key: String,
    pub codec: AudioCodec,
    pub bitrate_kbps: u32,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub trait AudioJobQueue {
    fn submit(&self, request: AudioJobRequest) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn get(&self, job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn mark_running(&self, job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn mark_completed(&self, job_id: &str) -> impl Future<Output = Result<AudioJob, AppError>>;
    fn mark_failed(
        &self,
        job_id: &str,
        error_summary: &str,
    ) -> impl Future<Output = Result<AudioJob, AppError>>;
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
