use crate::db::Db;
use aws_sdk_s3::operation::head_object::HeadObjectError;
use aws_sdk_s3::primitives::ByteStream;
use chrono::{Timelike, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

#[derive(Clone, PartialEq, Eq)]
pub struct MediaSyncConfig {
    pub enabled: bool,
    pub types: Vec<MediaType>,
    pub schedule_time: Option<chrono::NaiveTime>,
    pub schedule_tz: Option<Tz>,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub prefix: String,
    pub images_dir: String,
    pub audio_dir: String,
    pub epubs_dir: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaType {
    Images,
    Audio,
    Epubs,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaType::Images => "images",
            MediaType::Audio => "audio",
            MediaType::Epubs => "epubs",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "images" | "image" | "cover" => Some(MediaType::Images),
            "audio" => Some(MediaType::Audio),
            "epubs" | "epub" => Some(MediaType::Epubs),
            _ => None,
        }
    }

    fn allowed_exts(&self) -> &'static [&'static str] {
        match self {
            MediaType::Images => &["jpg", "jpeg", "png", "webp", "gif"],
            MediaType::Audio => &["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus", "webm"],
            MediaType::Epubs => &["epub", "pdf", "zip"],
        }
    }

    fn local_dir<'a>(&self, cfg: &'a MediaSyncConfig) -> &'a str {
        match self {
            MediaType::Images => &cfg.images_dir,
            MediaType::Audio => &cfg.audio_dir,
            MediaType::Epubs => &cfg.epubs_dir,
        }
    }

    fn content_type_for_ext(ext: &str) -> &'static str {
        match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "flac" => "audio/flac",
            "ogg" => "audio/ogg",
            "m4a" => "audio/mp4",
            "aac" => "audio/aac",
            "opus" => "audio/opus",
            "webm" => "audio/webm",
            "epub" => "application/epub+zip",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
    }
}

#[derive(Default)]
struct TypeReport {
    scanned: usize,
    uploaded: usize,
    skipped: usize,
    failed: usize,
    missing_local: usize,
}

#[derive(Serialize)]
pub struct MediaSyncSummary {
    pub ok: bool,
    pub message: String,
    pub uploaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub missing_local: usize,
    pub scanned: usize,
    pub types: Vec<String>,
    pub per_type: std::collections::BTreeMap<String, TypeReportPublic>,
}

#[derive(Serialize)]
pub struct TypeReportPublic {
    pub scanned: usize,
    pub uploaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub missing_local: usize,
}

impl From<TypeReport> for TypeReportPublic {
    fn from(r: TypeReport) -> Self {
        Self {
            scanned: r.scanned,
            uploaded: r.uploaded,
            skipped: r.skipped,
            failed: r.failed,
            missing_local: r.missing_local,
        }
    }
}

impl MediaSyncConfig {
    pub fn load(db: &Db, images_dir: String, audio_dir: String, epubs_dir: String) -> Self {
        let enabled =
            Self::setting(db, "media_sync.enabled", "MEDIA_SYNC_ENABLED", "false").to_lowercase();
        let enabled = enabled == "true" || enabled == "1";

        let types_str = Self::setting(db, "media_sync.types", "MEDIA_SYNC_TYPES", "epubs,audio");
        let types = parse_types(&types_str);

        let schedule_time = Self::setting(
            db,
            "media_sync.schedule_time",
            "MEDIA_SYNC_SCHEDULE_TIME",
            "",
        )
        .parse::<chrono::NaiveTime>()
        .ok();

        let schedule_tz = Self::setting(
            db,
            "media_sync.schedule_tz",
            "MEDIA_SYNC_SCHEDULE_TZ",
            "Asia/Tokyo",
        )
        .parse::<Tz>()
        .ok();

        let endpoint = {
            let v = Self::setting(db, "media_sync.s3_endpoint", "MEDIA_SYNC_S3_ENDPOINT", "");
            if v.is_empty() {
                Self::setting(db, "backup.s3_endpoint", "BACKUP_S3_ENDPOINT", "")
            } else {
                v
            }
        };
        let region = {
            let v = Self::setting(db, "media_sync.s3_region", "MEDIA_SYNC_S3_REGION", "");
            if v.is_empty() {
                Self::setting(db, "backup.s3_region", "BACKUP_S3_REGION", "us-east-1")
            } else {
                v
            }
        };
        let bucket = {
            let v = Self::setting(db, "media_sync.s3_bucket", "MEDIA_SYNC_S3_BUCKET", "");
            if v.is_empty() {
                Self::setting(db, "backup.s3_bucket", "BACKUP_S3_BUCKET", "")
            } else {
                v
            }
        };
        let access_key = {
            let v = Self::setting(
                db,
                "media_sync.s3_access_key",
                "MEDIA_SYNC_S3_ACCESS_KEY",
                "",
            );
            if v.is_empty() {
                Self::setting(db, "backup.s3_access_key", "BACKUP_S3_ACCESS_KEY", "")
            } else {
                v
            }
        };
        let secret_key = {
            let v = Self::setting(
                db,
                "media_sync.s3_secret_key",
                "MEDIA_SYNC_S3_SECRET_KEY",
                "",
            );
            if v.is_empty() {
                Self::setting(db, "backup.s3_secret_key", "BACKUP_S3_SECRET_KEY", "")
            } else {
                v
            }
        };
        let prefix = {
            let v = Self::setting(db, "media_sync.s3_prefix", "MEDIA_SYNC_S3_PREFIX", "");
            if v.is_empty() {
                Self::setting(db, "backup.s3_prefix", "BACKUP_S3_PREFIX", "")
            } else {
                v
            }
        };

        Self {
            enabled,
            types,
            schedule_time,
            schedule_tz,
            endpoint,
            region,
            bucket,
            access_key,
            secret_key,
            prefix,
            images_dir,
            audio_dir,
            epubs_dir,
        }
    }

    /// Validate user-supplied configuration. Returns Err if anything is
    /// missing or inconsistent; intended to be checked before triggering a
    /// sync (e.g. from the API).
    pub fn validate(&self) -> Result<(), String> {
        if self.types.is_empty() {
            return Err("media_sync.types is empty (allowed: images, audio, epubs)".to_string());
        }
        if self.bucket.trim().is_empty() {
            return Err(
                "S3 bucket is not configured (set media_sync.s3_bucket or backup.s3_bucket)"
                    .to_string(),
            );
        }
        if self.endpoint.trim().is_empty() {
            return Err(
                "S3 endpoint is not configured (set media_sync.s3_endpoint or backup.s3_endpoint)"
                    .to_string(),
            );
        }
        if self.access_key.trim().is_empty() || self.secret_key.trim().is_empty() {
            return Err(
                "S3 credentials are not configured (set media_sync.s3_access_key/secret_key or backup equivalents)"
                    .to_string(),
            );
        }
        if contains_traversal(&self.prefix) {
            return Err(format!(
                "S3 prefix contains path-traversal segments (.. or absolute): {:?}",
                self.prefix
            ));
        }
        Ok(())
    }

    fn setting(db: &Db, db_key: &str, env_key: &str, default: &str) -> String {
        db.get_setting(db_key)
            .or_else(|| std::env::var(env_key).ok())
            .unwrap_or_else(|| default.to_string())
    }
}

fn parse_types(input: &str) -> Vec<MediaType> {
    let mut out = Vec::new();
    for part in input.split(',') {
        if let Some(t) = MediaType::parse(part) {
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    out
}

/// Returns true if `input` contains a path-traversal segment (`..`) or an
/// absolute-path marker that could escape the S3 bucket root.
fn contains_traversal(input: &str) -> bool {
    for seg in input.split(|c| c == '/' || c == '\\') {
        if seg == ".." {
            return true;
        }
    }
    // Reject leading slashes that would produce absolute keys.
    let trimmed = input.trim_start();
    trimmed.starts_with('/') || trimmed.starts_with('\\')
}

/// Result of probing whether a key already exists on S3.
enum HeadProbe {
    /// Object exists; skip upload.
    Exists,
    /// Object is not present (NotFound / 404); safe to upload.
    Missing,
    /// Head probe failed for a non-NotFound reason (auth/network/5xx/etc);
    /// do NOT overwrite; record as failure.
    Inaccessible(String),
}

async fn probe_object(client: &aws_sdk_s3::Client, bucket: &str, key: &str) -> HeadProbe {
    match client.head_object().bucket(bucket).key(key).send().await {
        Ok(_) => HeadProbe::Exists,
        Err(e) => {
            // Capture HTTP status first (uses a borrow), then move `e` into
            // the service error variant.
            let status_code: Option<u16> = e.raw_response().map(|raw| raw.status().as_u16());
            let svc = e.into_service_error();
            if let HeadObjectError::NotFound(_) = svc {
                HeadProbe::Missing
            } else {
                let detail = match status_code {
                    Some(code) => format!("HTTP {} ({})", code, svc),
                    None => format!("{}", svc),
                };
                HeadProbe::Inaccessible(detail)
            }
        }
    }
}

/// Recursively walk `root` collecting files that match the allowed
/// extensions. Returns `(relative_path, absolute_path)` pairs where
/// relative_path is the path relative to `root`, using `/` as the
/// separator.
fn collect_files(root: &Path, allowed_exts: &[&str]) -> Vec<(String, PathBuf)> {
    if !root.exists() {
        return Vec::new();
    }
    let allowed: std::collections::HashSet<String> = allowed_exts
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if ft.is_dir() {
                stack.push(path);
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase());
            let Some(ext) = ext else {
                continue;
            };
            if !allowed.contains(&ext) {
                continue;
            }
            let rel = match path.strip_prefix(root) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let rel_str = join_components(rel);
            if rel_str.is_empty() {
                continue;
            }
            out.push((rel_str, path));
        }
    }
    out
}

/// Join `path`'s normal components using `/` as the separator. Returns
/// empty string if any component is `..`, a Windows prefix, or a root
/// (path-traversal guard).
fn join_components(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::Normal(s) => {
                let s = s.to_string_lossy();
                if s.is_empty() || s == "." || s == ".." {
                    return String::new();
                }
                parts.push(s.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return String::new();
            }
        }
    }
    parts.join("/")
}

fn build_s3_key(prefix: &str, subdir: &str, rel_path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let p = prefix.trim().trim_matches('/');
    if !p.is_empty() {
        parts.push(p.to_string());
    }
    parts.push(subdir.to_string());
    parts.push(rel_path.replace('\\', "/"));
    parts.join("/")
}

/// Run media sync and return a summary.
///
/// Caller is expected to have validated the config first (via
/// `MediaSyncConfig::validate`). Errors here typically indicate unexpected
/// internal/S3 errors (e.g. head_object on a credential/5xx/403/timeout, or
/// put_object errors). Per-file failures are reflected in the summary's
/// `failed` field rather than as a top-level `Err`.
pub async fn perform_media_sync(config: &MediaSyncConfig) -> Result<MediaSyncSummary, String> {
    let credentials = aws_sdk_s3::config::Credentials::new(
        &config.access_key,
        &config.secret_key,
        None,
        None,
        "dantalian",
    );

    let s3_config = aws_sdk_s3::Config::builder()
        .endpoint_url(&config.endpoint)
        .region(aws_sdk_s3::config::Region::new(config.region.clone()))
        .credentials_provider(aws_sdk_s3::config::SharedCredentialsProvider::new(
            credentials,
        ))
        .force_path_style(true)
        .build();

    let client = aws_sdk_s3::Client::from_conf(s3_config);

    let mut per_type: std::collections::BTreeMap<String, TypeReport> =
        std::collections::BTreeMap::new();
    let mut total_uploaded = 0usize;
    let mut total_skipped = 0usize;
    let mut total_failed = 0usize;
    let mut total_missing = 0usize;
    let mut total_scanned = 0usize;

    for media_type in &config.types {
        let mut report = TypeReport::default();
        let dir = media_type.local_dir(config);
        let allowed = media_type.allowed_exts();
        let root = Path::new(dir);

        let files = if root.exists() {
            collect_files(root, allowed)
        } else {
            tracing::warn!(
                "media_sync: local directory does not exist for type {}: {}",
                media_type.as_str(),
                dir
            );
            Vec::new()
        };

        report.scanned = files.len();
        total_scanned += files.len();

        for (rel_path, path) in files {
            let key = build_s3_key(&config.prefix, media_type.as_str(), &rel_path);

            match probe_object(&client, &config.bucket, &key).await {
                HeadProbe::Exists => {
                    report.skipped += 1;
                    total_skipped += 1;
                    continue;
                }
                HeadProbe::Missing => {
                    // fall through to upload
                }
                HeadProbe::Inaccessible(reason) => {
                    tracing::warn!(
                        "media_sync: head_object failed for {} (skipping upload): {}",
                        key,
                        reason
                    );
                    report.failed += 1;
                    total_failed += 1;
                    continue;
                }
            }

            let path_str = path.to_string_lossy().to_string();
            let body = match ByteStream::from_path(&path).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!("media_sync: failed to open local file {}: {}", path_str, e);
                    report.missing_local += 1;
                    total_missing += 1;
                    continue;
                }
            };

            let content_type = MediaType::content_type_for_ext(
                Path::new(&rel_path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or(""),
            );

            match client
                .put_object()
                .bucket(&config.bucket)
                .key(&key)
                .content_type(content_type)
                .body(body)
                .send()
                .await
            {
                Ok(_) => {
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    tracing::info!(
                        "media_sync: uploaded {}/{} ({} bytes)",
                        config.bucket,
                        key,
                        size
                    );
                    report.uploaded += 1;
                    total_uploaded += 1;
                }
                Err(e) => {
                    tracing::error!("media_sync: upload failed for {}: {}", key, e);
                    report.failed += 1;
                    total_failed += 1;
                }
            }
        }

        per_type.insert(media_type.as_str().to_string(), report);
    }

    let types_str: Vec<String> = config
        .types
        .iter()
        .map(|t| t.as_str().to_string())
        .collect();
    let ok = total_failed == 0;
    let message = if ok {
        "Media sync completed".to_string()
    } else {
        format!(
            "Media sync completed with {} failure(s) ({} uploaded, {} skipped, {} missing_local)",
            total_failed, total_uploaded, total_skipped, total_missing
        )
    };
    let summary = MediaSyncSummary {
        ok,
        message,
        uploaded: total_uploaded,
        skipped: total_skipped,
        failed: total_failed,
        missing_local: total_missing,
        scanned: total_scanned,
        types: types_str,
        per_type: per_type.into_iter().map(|(k, v)| (k, v.into())).collect(),
    };
    tracing::info!(
        "Media sync finished: scanned={}, uploaded={}, skipped={}, failed={}, missing_local={}, ok={}",
        summary.scanned,
        summary.uploaded,
        summary.skipped,
        summary.failed,
        summary.missing_local,
        summary.ok
    );
    Ok(summary)
}

fn next_schedule(schedule_time: chrono::NaiveTime, tz: Tz) -> chrono::DateTime<Utc> {
    let now_utc = Utc::now();
    let now_tz = now_utc.with_timezone(&tz);
    let today_local = now_tz
        .date_naive()
        .and_time(schedule_time)
        .and_local_timezone(tz)
        .earliest()
        .unwrap();
    let today_utc = today_local.with_timezone(&Utc);
    if today_utc > now_utc {
        today_utc
    } else {
        (now_tz.date_naive() + chrono::Duration::days(1))
            .and_time(schedule_time)
            .and_local_timezone(tz)
            .earliest()
            .unwrap()
            .with_timezone(&Utc)
    }
}

/// Result of a `chunked_sleep` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SleepOutcome {
    /// The total duration elapsed without interruption.
    Completed,
    /// `should_stop` returned true during polling — caller should re-load
    /// configuration and recompute scheduling.
    ConfigChanged,
}

/// Sleep up to `max_chunk` at a time, polling the `should_stop` predicate
/// between chunks. Returns `Completed` if the total duration elapsed, or
/// `ConfigChanged` if the predicate asked to stop early.
async fn chunked_sleep(
    max_chunk: Duration,
    total: Duration,
    mut should_stop: impl FnMut() -> bool,
) -> SleepOutcome {
    let mut remaining = total;
    while remaining > Duration::ZERO {
        if should_stop() {
            return SleepOutcome::ConfigChanged;
        }
        let step = remaining.min(max_chunk);
        tokio::time::sleep(step).await;
        remaining = remaining.saturating_sub(step);
    }
    SleepOutcome::Completed
}

/// Start the media-sync background worker. The worker always runs and
/// reads configuration from the DB on every iteration so that runtime
/// changes take effect within a few minutes. When `media_sync.enabled` is
/// false the worker simply idles.
pub fn start_scheduled_media_sync(
    db: Db,
    images_dir: String,
    audio_dir: String,
    epubs_dir: String,
) -> tokio::task::JoinHandle<()> {
    const IDLE_POLL: Duration = Duration::from_secs(60);
    const CONFIG_POLL: Duration = Duration::from_secs(60);
    const MAX_SCHEDULE_CHUNK: Duration = Duration::from_secs(300);
    const POST_RUN_DELAY: Duration = Duration::from_secs(5);

    tokio::spawn(async move {
        let mut enabled_logged = false;
        let mut last_schedule_target: Option<(chrono::NaiveTime, Tz)> = None;

        loop {
            let config = MediaSyncConfig::load(
                &db,
                images_dir.clone(),
                audio_dir.clone(),
                epubs_dir.clone(),
            );

            if !config.enabled {
                if enabled_logged {
                    tracing::info!("Media sync disabled; worker idling");
                    enabled_logged = false;
                    last_schedule_target = None;
                }
                tokio::time::sleep(IDLE_POLL).await;
                continue;
            }

            if !enabled_logged {
                tracing::info!(
                    "Media sync enabled (types: {})",
                    config
                        .types
                        .iter()
                        .map(|t| t.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                );
                enabled_logged = true;
            }

            // Validate before scheduling; never run a broken sync.
            if let Err(e) = config.validate() {
                tracing::warn!("Media sync config invalid, skipping scheduled run: {}", e);
                last_schedule_target = None;
                tokio::time::sleep(CONFIG_POLL).await;
                continue;
            }

            let (schedule_time, tz) = match (config.schedule_time, config.schedule_tz) {
                (Some(t), Some(tz)) => (t, tz),
                _ => {
                    tracing::warn!(
                        "Scheduled media sync is enabled but schedule_time or schedule_tz is not set; waiting for configuration"
                    );
                    last_schedule_target = None;
                    tokio::time::sleep(CONFIG_POLL).await;
                    continue;
                }
            };

            if last_schedule_target != Some((schedule_time, tz)) {
                tracing::info!(
                    "Scheduled media sync: daily at {:02}:{:02} ({})",
                    schedule_time.hour(),
                    schedule_time.minute(),
                    tz.name()
                );
                last_schedule_target = Some((schedule_time, tz));
            }

            let target = next_schedule(schedule_time, tz);
            let now = Utc::now();
            let wait = target.signed_duration_since(now);
            let snapshot = config.clone();
            let outcome = if wait > chrono::Duration::zero() {
                let wait_std = wait.to_std().unwrap_or(Duration::from_secs(60));
                tracing::debug!(
                    "Next scheduled media sync in {}s (at {})",
                    wait_std.as_secs(),
                    target.format("%Y-%m-%d %H:%M:%S UTC")
                );
                let db_for_poll = db.clone();
                let images_clone = images_dir.clone();
                let audio_clone = audio_dir.clone();
                let epubs_clone = epubs_dir.clone();
                let snap_for_poll = snapshot.clone();
                chunked_sleep(MAX_SCHEDULE_CHUNK, wait_std, || {
                    let cfg = MediaSyncConfig::load(
                        &db_for_poll,
                        images_clone.clone(),
                        audio_clone.clone(),
                        epubs_clone.clone(),
                    );
                    cfg != snap_for_poll
                })
                .await
            } else {
                SleepOutcome::Completed
            };

            match outcome {
                SleepOutcome::ConfigChanged => {
                    // Configuration changed during the wait. Re-read at the
                    // top of the loop without running a sync, so that the
                    // new schedule / new credentials are honored.
                    tracing::debug!("Media sync config changed during wait; skipping this run");
                    tokio::time::sleep(POST_RUN_DELAY).await;
                    continue;
                }
                SleepOutcome::Completed => {}
            }

            // Re-read once more before running: an admin may have disabled
            // the schedule or changed config right at the boundary.
            let config = MediaSyncConfig::load(
                &db,
                images_dir.clone(),
                audio_dir.clone(),
                epubs_dir.clone(),
            );
            if !config.enabled {
                continue;
            }
            if config.validate().is_err() {
                tracing::warn!("Media sync config became invalid by run time; skipping this run");
                continue;
            }
            if config != snapshot {
                tracing::debug!("Media sync config changed at run boundary; skipping this run");
                continue;
            }

            match perform_media_sync(&config).await {
                Ok(s) => {
                    tracing::info!(
                        "Scheduled media sync done: ok={} uploaded={} skipped={} failed={}",
                        s.ok,
                        s.uploaded,
                        s.skipped,
                        s.failed
                    );
                }
                Err(e) => {
                    tracing::error!("Scheduled media sync failed: {}", e);
                }
            }

            tokio::time::sleep(POST_RUN_DELAY).await;
        }
    })
}
