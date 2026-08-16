use dantalian::application::error::AppError;
use worker::{Env, Request, Response, Result, RouteContext, ScheduleContext, ScheduledEvent};

use crate::error::error_response;

pub async fn run(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    error_response(AppError::Conflict(
        "media synchronization is not available in Worker runtime".to_string(),
    ))
}

pub async fn run_scheduled(_event: ScheduledEvent, _env: Env, _ctx: ScheduleContext) -> Result<()> {
    Ok(())
}
