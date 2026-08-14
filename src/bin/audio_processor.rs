use std::{env, fs, time::Duration};

use fs2::available_space;

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
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const DEFAULT_LEASE_SECONDS: u64 = 900;
const MIN_DISK_HEADROOM_BYTES: u64 = 512 * 1024 * 1024;
const POLL_INTERVAL_SECONDS: u64 = 5;
const MAX_PROCESSOR_INPUT_BYTES: i64 = 3 * 1024 * 1024 * 1024;

#[derive(Clone)]
struct Config {
    worker_base_url: String,
    api_token: String,
    processor_id: String,
    audio_job_id: Option<String>,
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
            None if config.audio_job_id.is_some() || config.once => return Ok(()),
            None => tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await,
        }
        if config.audio_job_id.is_some() || config.once {
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
        let audio_job_id = env::var("DANTALIAN_AUDIO_JOB_ID").ok();
        if let Some(job_id) = &audio_job_id {
            if job_id.len() != 32 || !job_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("DANTALIAN_AUDIO_JOB_ID must be a 32-character hex id".to_string());
            }
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
            audio_job_id,
            lease_seconds,
            once: env::var("DANTALIAN_PROCESSOR_ONCE").as_deref() == Ok("1"),
            bucket,
            s3_client: aws_sdk_s3::Client::from_conf(sdk_config),
        })
    }
}

async fn claim(client: &Client, config: &Config) -> Result<Option<AudioJobClaim>, String> {
    let response = client
        .post(api_url(config, "claim"))
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
                .post(api_url(&config, "renew"))
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
    let workspace = env::temp_dir().join(format!("dantalian-audio-{}", claim.job.id));
    fs::create_dir_all(&workspace)
        .map_err(|_| ProcessError::Retryable("audio workspace creation failed"))?;
    let input_path = workspace.join("input");
    let output_path = workspace.join("output");

    let result = async {
        let metadata = config
            .s3_client
            .head_object()
            .bucket(&config.bucket)
            .key(&claim.job.input_object_key)
            .send()
            .await
            .map_err(|_| ProcessError::Retryable("input object metadata lookup failed"))?;
        let input_size = metadata
            .content_length()
            .ok_or(ProcessError::Permanent("input object size is unavailable"))?;
        if !(1..=MAX_PROCESSOR_INPUT_BYTES).contains(&input_size) {
            return Err(ProcessError::Permanent(
                "input object exceeds processor limit",
            ));
        }
        let required_space = (input_size as u64)
            .checked_add(MIN_DISK_HEADROOM_BYTES)
            .ok_or(ProcessError::Permanent(
                "input object disk requirement overflows",
            ))?;
        let available_space = available_space(&workspace)
            .map_err(|_| ProcessError::Retryable("audio disk headroom lookup failed"))?;
        if available_space < required_space {
            return Err(ProcessError::Retryable(
                "audio processor disk headroom is insufficient",
            ));
        }

        let object = config
            .s3_client
            .get_object()
            .bucket(&config.bucket)
            .key(&claim.job.input_object_key)
            .send()
            .await
            .map_err(|_| ProcessError::Retryable("input object download failed"))?;
        let mut reader = object
            .body
            .into_async_read()
            .take((MAX_PROCESSOR_INPUT_BYTES + 1) as u64);
        let mut file = tokio::fs::File::create(&input_path)
            .await
            .map_err(|_| ProcessError::Retryable("input workspace open failed"))?;
        let copied = tokio::io::copy(&mut reader, &mut file)
            .await
            .map_err(|_| ProcessError::Retryable("input object streaming failed"))?;
        file.flush()
            .await
            .map_err(|_| ProcessError::Retryable("input workspace flush failed"))?;
        if copied > MAX_PROCESSOR_INPUT_BYTES as u64 {
            return Err(ProcessError::Permanent(
                "input object exceeds processor limit",
            ));
        }

        let codec = claim.job.codec;
        let bitrate_kbps = claim.job.bitrate_kbps;
        let source_extension = extension(&claim.job.input_object_key)
            .map(str::to_string)
            .ok_or(ProcessError::Permanent("input object extension is invalid"))?;
        let job_id = claim.job.id.clone();
        let input_for_encoder = input_path.clone();
        let output_for_encoder = output_path.clone();
        tokio::task::spawn_blocking(move || match codec {
            AudioCodec::Opus => dantalian::audio_codec::encode_opus_file_with_bitrate(
                input_for_encoder,
                output_for_encoder,
                &source_extension,
                &job_id,
                bitrate_kbps,
            )
            .map_err(|_| ProcessError::Permanent("audio Opus encoding failed")),
            AudioCodec::Aac => dantalian::audio_codec::encode_aac_file_with_bitrate(
                input_for_encoder,
                output_for_encoder,
                &source_extension,
                bitrate_kbps,
            )
            .map_err(|_| ProcessError::Permanent("audio AAC encoding failed")),
        })
        .await
        .map_err(|_| ProcessError::Permanent("audio processor task failed"))??;

        let content_type = match codec {
            AudioCodec::Opus => "audio/ogg",
            AudioCodec::Aac => "audio/aac",
        };
        let output = ByteStream::from_path(&output_path)
            .await
            .map_err(|_| ProcessError::Retryable("output workspace read failed"))?;
        config
            .s3_client
            .put_object()
            .bucket(&config.bucket)
            .key(&claim.job.output_object_key)
            .content_type(content_type)
            .body(output)
            .send()
            .await
            .map_err(|_| ProcessError::Retryable("output object upload failed"))?;
        Ok(())
    }
    .await;

    match tokio::fs::remove_dir_all(&workspace).await {
        Ok(()) => result,
        Err(_) => Err(ProcessError::Retryable("audio workspace cleanup failed")),
    }
}

async fn complete(client: &Client, config: &Config, claim: &AudioJobClaim) -> Result<(), String> {
    let response = client
        .post(api_url(config, "complete"))
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
        .post(api_url(config, "fail"))
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

fn api_url(config: &Config, operation: &str) -> String {
    match config.audio_job_id.as_deref() {
        Some(job_id) => format!(
            "{}/api/internal/audio/jobs/{job_id}/{operation}",
            config.worker_base_url
        ),
        None => format!("{}/api/audio/jobs/{operation}", config.worker_base_url),
    }
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} is required"))
}
