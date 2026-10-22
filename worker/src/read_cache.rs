use worker::{Cache, Method, Request, Response, Result};

use crate::d1_read;

/// Cached reads are short lived. The TTL must stay below the 30 second
/// `dantalian_d1_primary` write window: otherwise an entry cached just before a
/// write could outlive the primary-read bypass and be served stale after it.
const READ_TTL_SECONDS: u32 = 15;

const CACHE_STATUS_HEADER: &str = "x-dantalian-cache";

pub(crate) struct CachePlan {
    url: String,
    ttl: u32,
}

/// Decide whether a GET request may be served from the colo cache.
///
/// Requests carrying an API token (local/staging clients, CI contracts) and
/// requests made shortly after a write (`dantalian_d1_primary` cookie) bypass
/// the cache so they always observe current data. `/api/health`,
/// `/api/internal/*`, job endpoints, and upload-session endpoints are never
/// cached.
pub(crate) fn plan(req: &Request) -> Result<Option<CachePlan>> {
    if req.method() != Method::Get {
        return Ok(None);
    }
    if req.headers().get("authorization")?.is_some() {
        return Ok(None);
    }
    if d1_read::prefers_primary(req) {
        return Ok(None);
    }
    let mut url = req.url()?;
    url.set_fragment(None);
    let path = url.path().to_owned();
    match path.as_str() {
        "/api/health" => return Ok(None),
        p if p.starts_with("/api/audio/jobs") => return Ok(None),
        p if p.starts_with("/api/internal/") => return Ok(None),
        p if p.starts_with("/api/uploads/") => return Ok(None),
        p if p.starts_with("/api/") => {}
        _ => return Ok(None),
    }
    Ok(Some(CachePlan {
        url: url.to_string(),
        ttl: READ_TTL_SECONDS,
    }))
}

pub(crate) async fn lookup(plan: &CachePlan) -> Option<Response> {
    let cache = Cache::default();
    match cache.get(plan.url.as_str(), true).await {
        Ok(Some(mut hit)) => {
            let _ = hit.headers_mut().set(CACHE_STATUS_HEADER, "hit");
            Some(hit)
        }
        _ => None,
    }
}

/// Store a successful JSON response in the colo cache.
///
/// The Cache API requires an explicit `max-age`/`s-maxage`; `Response::cloned`
/// keeps the original response intact for the client.
pub(crate) async fn store(plan: &CachePlan, response: &mut Response) {
    if response.status_code() != 200 {
        return;
    }
    let Ok(mut cached) = response.cloned() else {
        return;
    };
    let control = format!("public, max-age={}", plan.ttl);
    if cached.headers_mut().set("cache-control", &control).is_err() {
        return;
    }
    let _ = cached.headers_mut().set(CACHE_STATUS_HEADER, "miss");
    let _ = Cache::default().put(plan.url.as_str(), cached).await;
}
