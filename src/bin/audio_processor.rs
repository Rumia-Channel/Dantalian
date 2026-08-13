use std::{env, time::Duration};

use aws_sdk_s3::{
    config::{Credentials, Region},
    primitives::ByteStream,
};
use dantalian::ports::{
    audio_jobs::{AudioJobClaim, AudioJobFailure},
    object_storage::AudioCodec,
};
use reqwest::{Client, StatusCode};
use serde::Serialize;

const DEFAULT_LEASE_SECONDS: u64 = 900;
const POLL_INTERVAL_SECONDS: u64 = 5;

#[derive(Clone)]
struct Config {
    worker_base_url: String,
    api_token: String,
    processor_id: String,
    lease_seconds: u64,
    once: bool,
    bucket: String,
    s3_client: aws_sdk_s3::Client,
}

#[derive(Debug, Serialize)]
struct ClaimRequest<'a> {
    processor_id: &'a str,
    lease_seconds: u64,
}

#[derive(Debug, Serialize)]
struct ClaimBody<'a> {
    claim: &'a AudioJobClaim,
}

#[derive(Debug, Serialize)]
struct FailureBody<'a> {
    claim: &'a AudioJobClaim,
    failure: AudioJobFailure,
}

#[derive(Debug)]
enum ProcessError {
    Retryable(&'static str),
    Permanent(&'static str),
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let config = Config::from_env()?;
    let client = Client::new();
    loop {
        match claim(&client, &config).await? {
            Some(claim) => {
                log_event("audio_job.claimed", &config, Some(&claim), None, None);
                if let Err(error) = process_claim(&client, &config, claim.clone()).await {
                    log_event(
                        "audio_job.failed",
                        &config,
                        Some(&claim),
                        Some(error.class()),
                        Some(error.summary()),
                    );
                } else {
                    log_event("audio_job.completed", &config, Some(&claim), None, None);
                }
            }
            None if config.once => return Ok(()),
            None => tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await,
        }
        if config.once {
            return Ok(());
        }
    }
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let worker_base_url = required_env("DANTALIAN_WORKER_BASE_URL")?
            .trim_end_matches('/')
            .to_string();
        let api_token = required_env("DANTALIAN_API_TOKEN")?;
        let processor_id = env::var("DANTALIAN_PROCESSOR_ID")
            .unwrap_or_else(|_| format!("processor-{}", std::process::id()));
        if processor_id.is_empty() || processor_id.len() > 128 {
            return Err("DANTALIAN_PROCESSOR_ID must be 1-128 characters".to_string());
        }
        let lease_seconds = env::var("DANTALIAN_PROCESSOR_LEASE_SECONDS")
            .ok()
            .map(|value| value.parse::<u64>())
            .transpose()
            .map_err(|_| "DANTALIAN_PROCESSOR_LEASE_SECONDS must be an integer".to_string())?
            .unwrap_or(DEFAULT_LEASE_SECONDS)
            .clamp(30, 3_600);
        let bucket = required_env("WASABI_BUCKET")?;
        let access_key = required_env("WASABI_ACCESS_KEY_ID")?;
        let secret_key = required_env("WASABI_SECRET_ACCESS_KEY")?;
        let endpoint = required_env("WASABI_ENDPOINT")?;
        let region = required_env("WASABI_REGION")?;
        let credentials = Credentials::new(
            access_key,
            secret_key,
            None,
            None,
            "dantalian-audio-processor",
        );
        let sdk_config = aws_sdk_s3::Config::builder()
            .endpoint_url(endpoint)
            .region(Region::new(region))
            .credentials_provider(credentials)
            .force_path_style(true)
            .build();
        Ok(Self {
            worker_base_url,
            api_token,
            processor_id,
            lease_seconds,
            once: env::var("DANTALIAN_PROCESSOR_ONCE").as_deref() == Ok("1"),
            bucket,
            s3_client: aws_sdk_s3::Client::from_conf(sdk_config),
        })
    }
}

async fn claim(client: &Client, config: &Config) -> Result<Option<AudioJobClaim>, String> {
    let response = client
        .post(format!("{}/api/audio/jobs/claim", config.worker_base_url))
        .bearer_auth(&config.api_token)
        .json(&ClaimRequest {
            processor_id: &config.processor_id,
            lease_seconds: config.lease_seconds,
        })
        .send()
        .await
        .map_err(|_| "audio processor claim request failed".to_string())?;
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err("audio processor claim was rejected".to_string());
    }
    let body = response
        .json::<AudioJobClaim>()
        .await
        .map_err(|_| "audio processor claim response was invalid".to_string())?;
    Ok(Some(body))
}

async fn process_claim(
    client: &Client,
    config: &Config,
    claim: AudioJobClaim,
) -> Result<(), ProcessError> {
    let heartbeat = spawn_heartbeat(client.clone(), config.clone(), claim.clone());
    let result = process_object(config, &claim).await;
    heartbeat.abort();
    match result {
        Ok(()) => complete(client, config, &claim)
            .await
            .map_err(|_| ProcessError::Retryable("worker completion update failed")),
        Err(error) => {
            let failure = AudioJobFailure {
                error_summary: error.summary().to_string(),
                retryable: matches!(error, ProcessError::Retryable(_)),
                backoff_seconds: if matches!(error, ProcessError::Retryable(_)) {
                    60
                } else {
                    0
                },
            };
            fail(client, config, &claim, failure)
                .await
                .map_err(|_| ProcessError::Retryable("worker failure update failed"))
                .map(|_| ())
        }
    }
}

fn spawn_heartbeat(
    client: Client,
    config: Config,
    claim: AudioJobClaim,
) -> tokio::task::JoinHandle<()> {
    let interval = Duration::from_secs((config.lease_seconds / 3).clamp(30, 300));
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            let result = client
                .post(format!("{}/api/audio/jobs/renew", config.worker_base_url))
                .bearer_auth(&config.api_token)
                .json(&serde_json::json!({
                    "claim": &claim,
                    "lease_seconds": config.lease_seconds,
                }))
                .send()
                .await;
            if !matches!(result, Ok(response) if response.status().is_success()) {
                return;
            }
        }
    })
}

async fn process_object(config: &Config, claim: &AudioJobClaim) -> Result<(), ProcessError> {
    let input = config
        .s3_client
        .get_object()
        .bucket(&config.bucket)
        .key(&claim.job.input_object_key)
        .send()
        .await
        .map_err(|_| ProcessError::Retryable("input object download failed"))?
        .body
        .collect()
        .await
        .map_err(|_| ProcessError::Retryable("input object read failed"))?
        .into_bytes()
        .to_vec();
    let codec = claim.job.codec;
    let source_extension = extension(&claim.job.input_object_key)
        .map(str::to_string)
        .ok_or(ProcessError::Permanent("input object extension is invalid"))?;
    let job_id = claim.job.id.clone();
    let encoded = tokio::task::spawn_blocking(move || match codec {
        AudioCodec::Opus => dantalian::audio_codec::encode_opus(&input, &source_extension, &job_id)
            .map_err(|_| ProcessError::Permanent("audio Opus encoding failed")),
        AudioCodec::Aac => dantalian::audio_codec::encode_aac(&input, &source_extension)
            .map_err(|_| ProcessError::Permanent("audio AAC encoding failed")),
    })
    .await
    .map_err(|_| ProcessError::Permanent("audio processor task failed"))??;
    let content_type = match codec {
        AudioCodec::Opus => "audio/ogg",
        AudioCodec::Aac => "audio/aac",
    };
    config
        .s3_client
        .put_object()
        .bucket(&config.bucket)
        .key(&claim.job.output_object_key)
        .content_type(content_type)
        .body(ByteStream::from(encoded))
        .send()
        .await
        .map_err(|_| ProcessError::Retryable("output object upload failed"))?;
    Ok(())
}

async fn complete(client: &Client, config: &Config, claim: &AudioJobClaim) -> Result<(), String> {
    let response = client
        .post(format!(
            "{}/api/audio/jobs/complete",
            config.worker_base_url
        ))
        .bearer_auth(&config.api_token)
        .json(&ClaimBody { claim })
        .send()
        .await
        .map_err(|_| "worker completion request failed".to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err("worker rejected audio completion".to_string())
    }
}

async fn fail(
    client: &Client,
    config: &Config,
    claim: &AudioJobClaim,
    failure: AudioJobFailure,
) -> Result<(), String> {
    let response = client
        .post(format!("{}/api/audio/jobs/fail", config.worker_base_url))
        .bearer_auth(&config.api_token)
        .json(&FailureBody { claim, failure })
        .send()
        .await
        .map_err(|_| "worker failure request failed".to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err("worker rejected audio failure".to_string())
    }
}

fn log_event(
    event: &str,
    config: &Config,
    claim: Option<&AudioJobClaim>,
    error_class: Option<&str>,
    error_summary: Option<&str>,
) {
    let mut payload = serde_json::json!({
        "event": event,
        "processor_id": config.processor_id,
    });
    if let Some(claim) = claim {
        payload["job_id"] = serde_json::json!(claim.job.id);
        payload["status"] = serde_json::json!(claim.job.status);
        payload["attempt"] = serde_json::json!(claim.job.attempt_count);
    }
    if let Some(error_class) = error_class {
        payload["error_class"] = serde_json::json!(error_class);
    }
    if let Some(error_summary) = error_summary {
        payload["error_summary"] = serde_json::json!(error_summary);
    }
    eprintln!("{payload}");
}

impl ProcessError {
    fn summary(&self) -> &'static str {
        match self {
            Self::Retryable(summary) | Self::Permanent(summary) => summary,
        }
    }
    fn class(&self) -> &'static str {
        match self {
            Self::Retryable(_) => "retryable",
            Self::Permanent(_) => "permanent",
        }
    }
}

fn extension(key: &str) -> Option<&str> {
    key.rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| !extension.is_empty() && extension.len() <= 10)
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} is required"))
}
