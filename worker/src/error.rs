use dantalian::application::error::AppError;
use serde::de::DeserializeOwned;
use worker::{Request, Response, Result, RouteContext};

pub fn error_response(error: AppError) -> Result<Response> {
    let status = status_for_error(&error);
    Response::from_json(&serde_json::json!({ "error": error.to_string() }))
        .map(|response| response.with_status(status))
}

fn status_for_error(error: &AppError) -> u16 {
    match error {
        AppError::Validation(_) => 400,
        AppError::NotFound => 404,
        AppError::Conflict(_) => 409,
        AppError::Database(_) | AppError::Storage(_) | AppError::Internal(_) => 500,
    }
}

pub fn bad_request(message: impl Into<String>) -> Response {
    Response::from_json(&serde_json::json!({ "error": message.into() }))
        .expect("serializing a string error cannot fail")
        .with_status(400)
}

pub fn parse_id(ctx: &RouteContext<()>, name: &str) -> std::result::Result<i64, Response> {
    let Some(raw) = ctx.param(name) else {
        return Err(bad_request(format!("missing {name}")));
    };
    raw.parse::<i64>()
        .map_err(|_| bad_request(format!("invalid {name}")))
}

pub async fn parse_json<T: DeserializeOwned>(
    req: &mut Request,
) -> std::result::Result<T, Response> {
    req.json()
        .await
        .map_err(|error| bad_request(format!("invalid JSON: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_status_contract_is_stable() {
        assert_eq!(
            status_for_error(&AppError::Validation("bad".to_string())),
            400
        );
        assert_eq!(status_for_error(&AppError::NotFound), 404);
        assert_eq!(
            status_for_error(&AppError::Conflict("duplicate".to_string())),
            409
        );
        assert_eq!(status_for_error(&AppError::Database("db".to_string())), 500);
        assert_eq!(
            status_for_error(&AppError::Storage("storage".to_string())),
            500
        );
    }
}
