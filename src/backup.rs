use crate::db::Db;
use chrono::{Datelike, Timelike, Utc};
use chrono_tz::Tz;
use std::path::Path;
use std::time::Duration;

#[derive(Clone)]
pub struct BackupConfig {
    pub enabled: bool,
    pub schedule_time: Option<chrono::NaiveTime>,
    pub schedule_tz: Option<Tz>,
    pub dest: BackupDestination,
    pub retention: usize,
}

#[derive(Clone)]
pub enum BackupDestination {
    Local {
        path: String,
    },
    WebDAV {
        url: String,
        username: String,
        password: String,
    },
    S3 {
        endpoint: String,
        region: String,
        bucket: String,
        access_key: String,
        secret_key: String,
        prefix: String,
    },
}

impl BackupConfig {
    pub fn load(db: &Db) -> Self {
        let enabled = Self::setting(db, "backup.enabled", "BACKUP_ENABLED", "false")
            .to_lowercase();
        let enabled = enabled == "true" || enabled == "1";

        let schedule_time = Self::setting(db, "backup.schedule_time", "BACKUP_SCHEDULE_TIME", "")
            .parse::<chrono::NaiveTime>()
            .ok();

        let schedule_tz = Self::setting(db, "backup.schedule_tz", "BACKUP_SCHEDULE_TZ", "")
            .parse::<Tz>()
            .ok();

        let retention: usize = Self::setting(db, "backup.retention", "BACKUP_RETENTION", "7")
            .parse()
            .unwrap_or(7);

        let dest_type = Self::setting(db, "backup.dest_type", "BACKUP_DEST_TYPE", "local");

        let dest = match dest_type.as_str() {
            "webdav" => BackupDestination::WebDAV {
                url: Self::setting(db, "backup.webdav_url", "BACKUP_WEBDAV_URL", ""),
                username: Self::setting(db, "backup.webdav_user", "BACKUP_WEBDAV_USER", ""),
                password: Self::setting(db, "backup.webdav_pass", "BACKUP_WEBDAV_PASS", ""),
            },
            "s3" => BackupDestination::S3 {
                endpoint: Self::setting(db, "backup.s3_endpoint", "BACKUP_S3_ENDPOINT", ""),
                region: Self::setting(db, "backup.s3_region", "BACKUP_S3_REGION", "us-east-1"),
                bucket: Self::setting(db, "backup.s3_bucket", "BACKUP_S3_BUCKET", ""),
                access_key: Self::setting(
                    db,
                    "backup.s3_access_key",
                    "BACKUP_S3_ACCESS_KEY",
                    "",
                ),
                secret_key: Self::setting(
                    db,
                    "backup.s3_secret_key",
                    "BACKUP_S3_SECRET_KEY",
                    "",
                ),
                prefix: Self::setting(db, "backup.s3_prefix", "BACKUP_S3_PREFIX", ""),
            },
            _ => {
                let default_path = std::env::var("DATA_DIR").unwrap_or_else(|_| {
                    dirs::document_dir()
                        .or_else(dirs::data_dir)
                        .unwrap()
                        .join("Dantalian")
                        .to_string_lossy()
                        .to_string()
                });
                let default_path = format!("{}/backups", default_path);
                BackupDestination::Local {
                    path: Self::setting(
                        db,
                        "backup.local_path",
                        "BACKUP_LOCAL_PATH",
                        &default_path,
                    ),
                }
            }
        };

        Self {
            enabled,
            schedule_time,
            schedule_tz,
            dest,
            retention,
        }
    }

    fn setting(db: &Db, db_key: &str, env_key: &str, default: &str) -> String {
        db.get_setting(db_key)
            .or_else(|| std::env::var(env_key).ok())
            .unwrap_or_else(|| default.to_string())
    }
}

fn make_filename() -> String {
    let now = Utc::now();
    format!(
        "dantalian-{:04}-{:02}-{:02}T{:02}-{:02}-{:02}Z.db",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

pub async fn perform_backup(db: &Db, config: &BackupConfig) {
    tracing::info!("Starting database backup");

    let filename = make_filename();
    let temp_dir = std::env::temp_dir();
    let temp_dir_str = temp_dir.to_string_lossy().to_string();
    let temp_path = temp_dir.join(&filename);
    let temp_path_str = temp_path.to_string_lossy().to_string();

    let db = db.clone();
    let temp_path_clone = temp_path_str.clone();

    let result = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&temp_dir_str).ok();
        db.backup_to_file(&temp_path_clone)
    })
    .await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            tracing::error!("Failed to create local backup: {}", e);
            let _ = tokio::fs::remove_file(&temp_path_str).await;
            return;
        }
        Err(e) => {
            tracing::error!("Backup task panicked: {}", e);
            return;
        }
    }

    match &config.dest {
        BackupDestination::Local { path } => {
            let dest_dir = Path::new(path);
            if let Err(e) = tokio::fs::create_dir_all(dest_dir).await {
                tracing::error!("Failed to create backup directory {}: {}", path, e);
                let _ = tokio::fs::remove_file(&temp_path_str).await;
                return;
            }
            let dest_path = dest_dir.join(&filename);
            if let Err(e) = tokio::fs::copy(&temp_path_str, &dest_path).await {
                tracing::error!("Failed to copy backup to {}: {}", dest_path.display(), e);
                let _ = tokio::fs::remove_file(&temp_path_str).await;
                return;
            }
            tracing::info!("Backup saved to {}", dest_path.display());
        }
        BackupDestination::WebDAV {
            url,
            username,
            password,
        } => {
            upload_webdav(&temp_path_str, url, &filename, username, password).await;
        }
        BackupDestination::S3 {
            endpoint,
            region,
            bucket,
            access_key,
            secret_key,
            prefix,
        } => {
            upload_s3(
                &temp_path_str,
                endpoint,
                region,
                bucket,
                access_key,
                secret_key,
                prefix,
                &filename,
            )
            .await;
        }
    }

    let _ = tokio::fs::remove_file(&temp_path_str).await;

    cleanup_old_backups(config, &filename).await;
}

async fn upload_webdav(
    temp_path: &str,
    base_url: &str,
    filename: &str,
    username: &str,
    password: &str,
) {
    let url = if base_url.ends_with('/') {
        format!("{}{}", base_url, filename)
    } else {
        format!("{}/{}", base_url, filename)
    };

    let data = match tokio::fs::read(temp_path).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to read temp backup file: {}", e);
            return;
        }
    };

    let client = reqwest::Client::builder()
        .build()
        .unwrap_or_default();

    match client
        .put(&url)
        .basic_auth(username, Some(password))
        .body(data)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("Backup uploaded to WebDAV: {}", url);
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("WebDAV upload failed: {} {} - {}", status, url, body);
        }
        Err(e) => {
            tracing::error!("WebDAV upload error for {}: {}", url, e);
        }
    }
}

async fn upload_s3(
    temp_path: &str,
    endpoint: &str,
    region: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    prefix: &str,
    filename: &str,
) {
    let key = if prefix.is_empty() {
        filename.to_string()
    } else if prefix.ends_with('/') {
        format!("{}{}", prefix, filename)
    } else {
        format!("{}/{}", prefix, filename)
    };

    let credentials = aws_sdk_s3::config::Credentials::new(
        access_key,
        secret_key,
        None,
        None,
        "dantalian",
    );

    let s3_config = aws_sdk_s3::Config::builder()
        .endpoint_url(endpoint)
        .region(aws_sdk_s3::config::Region::new(region.to_string()))
        .credentials_provider(aws_sdk_s3::config::SharedCredentialsProvider::new(
            credentials,
        ))
        .force_path_style(true)
        .build();

    let client = aws_sdk_s3::Client::from_conf(s3_config);

    let body = match aws_sdk_s3::primitives::ByteStream::from_path(temp_path).await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Failed to read temp backup file for S3: {}", e);
            return;
        }
    };

    match client
        .put_object()
        .bucket(bucket)
        .key(&key)
        .body(body)
        .send()
        .await
    {
        Ok(_) => {
            tracing::info!("Backup uploaded to S3: {}/{}", bucket, key);
        }
        Err(e) => {
            tracing::error!("S3 upload failed: {}", e);
        }
    }
}

async fn cleanup_old_backups(config: &BackupConfig, current_filename: &str) {
    match &config.dest {
        BackupDestination::Local { path } => {
            cleanup_local(path, config.retention, current_filename).await;
        }
        BackupDestination::WebDAV {
            url,
            username,
            password,
        } => {
            cleanup_webdav(url, username, password, config.retention, current_filename).await;
        }
        BackupDestination::S3 {
            endpoint,
            region,
            bucket,
            access_key,
            secret_key,
            prefix,
        } => {
            cleanup_s3(
                endpoint,
                region,
                bucket,
                access_key,
                secret_key,
                prefix,
                config.retention,
                current_filename,
            )
            .await;
        }
    }
}

async fn cleanup_local(dir: &str, retention: usize, current_filename: &str) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };

    let mut backups: Vec<String> = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("dantalian-") && name.ends_with(".db") && name != current_filename {
            backups.push(name);
        }
    }

    backups.sort();

    if backups.len() <= retention {
        return;
    }

    let to_delete = backups.len() - retention;
    for name in &backups[..to_delete] {
        let path = Path::new(dir).join(name);
        if let Err(e) = tokio::fs::remove_file(&path).await {
            tracing::warn!("Failed to delete old backup {}: {}", path.display(), e);
        } else {
            tracing::info!("Deleted old backup: {}", path.display());
        }
    }
}

async fn cleanup_webdav(
    base_url: &str,
    username: &str,
    password: &str,
    retention: usize,
    current_filename: &str,
) {
    let backups = list_webdav_backups(base_url, username, password).await;
    if backups.len() <= retention {
        return;
    }

    let mut sorted = backups;
    sorted.sort();

    let to_delete = sorted.len().saturating_sub(retention);
    let client = reqwest::Client::builder()
        .build()
        .unwrap_or_default();

    for name in sorted.iter().take(to_delete) {
        if name == current_filename {
            continue;
        }
        let url = if base_url.ends_with('/') {
            format!("{}{}", base_url, name)
        } else {
            format!("{}/{}", base_url, name)
        };
        match client
            .delete(&url)
            .basic_auth(username, Some(password))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Deleted old backup from WebDAV: {}", url);
            }
            Ok(resp) => {
                tracing::warn!(
                    "Failed to delete old backup from WebDAV {}: {}",
                    url,
                    resp.status()
                );
            }
            Err(e) => {
                tracing::warn!("Failed to delete old backup from WebDAV {}: {}", url, e);
            }
        }
    }
}

async fn list_webdav_backups(base_url: &str, username: &str, password: &str) -> Vec<String> {
    let url = if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{}/", base_url)
    };

    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:resourcetype/>
    <D:getlastmodified/>
  </D:prop>
</D:propfind>"#;

    let client = reqwest::Client::builder()
        .build()
        .unwrap_or_default();

    let resp = match client
        .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
        .basic_auth(username, Some(password))
        .header("Depth", "1")
        .header("Content-Type", "application/xml")
        .body(body.to_string())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("WebDAV PROPFIND failed: {}", e);
            return Vec::new();
        }
    };

    if !resp.status().is_success() {
        tracing::warn!("WebDAV PROPFIND returned {}", resp.status());
        return Vec::new();
    }

    let xml = match resp.text().await {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut backups = Vec::new();
    for line in xml.lines() {
        let Some(start) = line.find("<D:href>") else {
            continue;
        };
        let rest = &line[start + "<D:href>".len()..];
        let Some(end) = rest.find("</D:href>") else {
            continue;
        };
        let href = &rest[..end];
        let parts: Vec<&str> = href.rsplitn(2, '/').collect();
        let filename = parts[0];
        if filename.starts_with("dantalian-")
            && filename.ends_with(".db")
            && !filename.is_empty()
        {
            backups.push(filename.to_string());
        }
    }

    backups
}

async fn cleanup_s3(
    endpoint: &str,
    region: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    prefix: &str,
    retention: usize,
    current_filename: &str,
) {
    let credentials = aws_sdk_s3::config::Credentials::new(
        access_key,
        secret_key,
        None,
        None,
        "dantalian",
    );

    let s3_config = aws_sdk_s3::Config::builder()
        .endpoint_url(endpoint)
        .region(aws_sdk_s3::config::Region::new(region.to_string()))
        .credentials_provider(aws_sdk_s3::config::SharedCredentialsProvider::new(
            credentials,
        ))
        .force_path_style(true)
        .build();

    let client = aws_sdk_s3::Client::from_conf(s3_config);

    let search_prefix = match prefix {
        p if p.is_empty() => "dantalian-".to_string(),
        p if p.ends_with('/') => format!("{}dantalian-", p),
        p => format!("{}/dantalian-", p),
    };

    let result = client
        .list_objects_v2()
        .bucket(bucket)
        .prefix(&search_prefix)
        .send()
        .await;

    let response = match result {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("S3 list_objects failed: {}", e);
            return;
        }
    };

    let mut keys: Vec<String> = Vec::new();
    for obj in response.contents() {
        if let Some(key) = obj.key() {
            let parts: Vec<&str> = key.rsplitn(2, '/').collect();
            let filename = parts[0];
            if filename.starts_with("dantalian-") && filename.ends_with(".db") {
                keys.push(key.to_string());
            }
        }
    }

    keys.sort();

    if keys.len() <= retention {
        return;
    }

    let to_delete = keys.len() - retention;
    for key in keys.iter().take(to_delete) {
        if key.ends_with(current_filename) {
            continue;
        }
        match client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => {
                tracing::info!("Deleted old backup from S3: {}/{}", bucket, key);
            }
            Err(e) => {
                tracing::warn!("Failed to delete old S3 backup {}: {}", key, e);
            }
        }
    }
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

pub fn start_scheduled_backup(db: Db) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut started = false;

        loop {
            let config = BackupConfig::load(&db);

            if !config.enabled {
                tokio::time::sleep(Duration::from_secs(300)).await;
                continue;
            }

            let (schedule_time, tz) = match (config.schedule_time, config.schedule_tz) {
                (Some(t), Some(tz)) => (t, tz),
                _ => {
                    if !started {
                        tracing::warn!(
                            "Scheduled backup is enabled but schedule_time or schedule_tz is not set"
                        );
                        started = true;
                    }
                    tokio::time::sleep(Duration::from_secs(300)).await;
                    continue;
                }
            };

            if !started {
                tracing::info!(
                    "Scheduled backup enabled: daily at {:02}:{:02} ({})",
                    schedule_time.hour(),
                    schedule_time.minute(),
                    tz.name()
                );
                started = true;
            }

            let target = next_schedule(schedule_time, tz);
            let now = Utc::now();
            let wait = target.signed_duration_since(now);
            if wait > chrono::Duration::zero() {
                let wait = wait.to_std().unwrap_or(Duration::from_secs(60));
                tracing::debug!(
                    "Next scheduled backup in {}s (at {})",
                    wait.as_secs(),
                    target.format("%Y-%m-%d %H:%M:%S UTC")
                );
                tokio::time::sleep(wait).await;
            }

            let config = BackupConfig::load(&db);
            if config.enabled {
                perform_backup(&db, &config).await;
            }

            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    })
}
