use worker::{Request, Response, Result, RouteContext};

const AUDIO_EXTERNAL_PROCESSING_STATUS: u16 = 501;

/// Audio transcoding is intentionally outside the Worker boundary.
///
/// The native implementation performs full-input decoding and buffering. The
/// Worker keeps this route as an explicit contract so callers can dispatch the
/// job to the external processor instead of silently attempting it in WASM.
pub async fn encode(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::from_json(&serde_json::json!({
        "error": "audio processing requires the external processor",
        "code": "audio_processing_external_required",
    }))
    .map(|response| response.with_status(AUDIO_EXTERNAL_PROCESSING_STATUS))
}
