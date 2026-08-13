use serde::Serialize;
use worker::{
    D1Database, Env, Request, Response, Result, RouteContext, ScheduleContext, ScheduledEvent,
};

use crate::{
    error::error_response,
    wasabi::{WasabiConfig, WasabiStorage},
};
use dantalian::application::error::AppError;
use dantalian::ports::object_storage::ObjectStorage;
#[derive(Debug, Serialize)]
pub(crate) struct SyncResult {
    ok: bool,
    enabled: bool,
    source: &'static str,
    scanned: u64,
    uploaded: u64,
    skipped: u64,
    failed: u64,
    missing_local: u64,
    message: &'static str,
}

pub async fn run(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    match sync(ctx.d1("DB")?, &ctx.env).await {
        Ok(result) => Response::from_json(&result),
        Err(error) => error_response(error),
    }
}

pub async fn run_scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) -> Result<()> {
    let result = sync(env.d1("DB")?, &env)
        .await
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    if !result.ok {
        return Err(worker::Error::RustError(
            "scheduled media synchronization failed".to_string(),
        ));
    }
    Ok(())
}

async fn sync(db: D1Database, env: &Env) -> std::result::Result<SyncResult, AppError> {
    let enabled = db
        .prepare("SELECT value FROM settings WHERE key = 'media_sync.enabled'")
        .first::<serde_json::Value>(None)
        .await
        .map_err(|error| AppError::Database(error.to_string()))?
        .and_then(|row| {
            row.get("value")
                .and_then(|value| value.as_str())
                .map(|value| matches!(value, "true" | "1"))
        })
        .unwrap_or(false);
    if !enabled {
        return Ok(SyncResult {
            ok: true,
            enabled: false,
            source: "cloudflare-object-storage",
            scanned: 0,
            uploaded: 0,
            skipped: 0,
            failed: 0,
            missing_local: 0,
            message: "media is uploaded directly to object storage",
        });
    }
    let config =
        WasabiConfig::from_env(env).map_err(|error| AppError::Storage(error.to_string()))?;
    let storage = WasabiStorage::new(config);
    let rows = db
        .prepare("SELECT object_key FROM object_uploads WHERE status = 'complete'")
        .all()
        .await
        .map_err(|error| AppError::Database(error.to_string()))?
        .results::<serde_json::Value>()
        .map_err(|error| AppError::Database(error.to_string()))?;
    let scanned = rows.len() as u64;
    let mut skipped = 0;
    let mut failed = 0;
    for row in rows {
        let Some(key) = row.get("object_key").and_then(|value| value.as_str()) else {
            failed += 1;
            continue;
        };
        match storage.exists(key).await {
            Ok(true) => skipped += 1,
            Ok(false) | Err(_) => failed += 1,
        }
    }
    Ok(SyncResult {
        ok: failed == 0,
        enabled: true,
        source: "cloudflare-object-storage",
        scanned,
        uploaded: 0,
        skipped,
        failed,
        missing_local: 0,
        message: "media is uploaded directly to object storage; no local files are synchronized",
    })
}
