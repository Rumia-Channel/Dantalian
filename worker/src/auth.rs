use worker::{Env, Request, Response, Result};

const TOKEN_SECRET: &str = "DANTALIAN_API_TOKEN";
const REQUIRED_VAR: &str = "DANTALIAN_AUTH_REQUIRED";
const DEV_VAR: &str = "DANTALIAN_DEV_MODE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthDecision {
    Allow,
    Reject,
    ConfigurationError,
}

fn env_string(env: &Env, name: &str) -> Option<String> {
    env.secret(name)
        .or_else(|_| env.var(name))
        .ok()
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
}

fn is_true(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
}

fn constant_time_equal(left: &str, right: &str) -> bool {
    let mut difference = left.len() ^ right.len();
    for (left, right) in left.bytes().zip(right.bytes()) {
        difference |= usize::from(left ^ right);
    }
    difference == 0
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = (bytes[index + 1] as char).to_digit(16)?;
            let low = (bytes[index + 2] as char).to_digit(16)?;
            decoded.push((high * 16 + low) as u8);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn cookie_value(cookie_header: &str, name: &str) -> Option<String> {
    cookie_header.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| {
            let value = value.trim();
            if value.contains('%') {
                percent_decode(value).unwrap_or_else(|| value.to_string())
            } else {
                value.to_string()
            }
        })
    })
}

fn bearer_value(authorization: &str) -> Option<&str> {
    let (scheme, token) = authorization.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(token.trim())
        .filter(|token| !token.is_empty())
}

fn requires_auth(path: &str) -> bool {
    path.starts_with("/api/")
        || path.starts_with("/audio/")
        || path.starts_with("/images/")
        || path.starts_with("/epubs/")
}

fn decision(env: &Env, request: &Request) -> Result<AuthDecision> {
    let token = env_string(env, TOKEN_SECRET);
    let dev_mode = is_true(env_string(env, DEV_VAR).as_deref());
    let required = env_string(env, REQUIRED_VAR)
        .map(|value| is_true(Some(&value)))
        .unwrap_or(!dev_mode);

    if token.is_none() {
        return Ok(if required {
            AuthDecision::ConfigurationError
        } else {
            AuthDecision::Allow
        });
    }

    let supplied = request
        .headers()
        .get("authorization")?
        .as_deref()
        .and_then(bearer_value)
        .map(str::to_owned)
        .or_else(|| {
            request
                .headers()
                .get("cookie")
                .ok()
                .flatten()
                .and_then(|header| cookie_value(&header, "dantalian_api_token"))
        });

    Ok(match supplied {
        Some(supplied) if constant_time_equal(&supplied, token.as_deref().unwrap_or_default()) => {
            AuthDecision::Allow
        }
        _ => AuthDecision::Reject,
    })
}

pub fn authorize(env: &Env, request: &Request) -> Result<Option<Response>> {
    if !requires_auth(&request.path()) {
        return Ok(None);
    }

    if request.path() == "/api/health" {
        return Ok(None);
    }

    match decision(env, request)? {
        AuthDecision::Allow => Ok(None),
        AuthDecision::Reject => {
            let mut response = Response::from_json(&serde_json::json!({
                "error": "authentication required",
                "code": "authentication_required",
            }))?
            .with_status(401);
            response.headers_mut().set("www-authenticate", "Bearer")?;
            Ok(Some(response))
        }
        AuthDecision::ConfigurationError => Ok(Some(
            Response::from_json(&serde_json::json!({
                "error": "Worker authentication is not configured",
                "code": "authentication_not_configured",
            }))?
            .with_status(500),
        )),
    }
}

const PROCESSOR_TOKEN_SECRET: &str = "DANTALIAN_PROCESSOR_TOKEN";

pub fn authorize_processor(env: &Env, request: &Request) -> Result<Option<Response>> {
    let token = env_string(env, PROCESSOR_TOKEN_SECRET);
    let authorization = request.headers().get("authorization")?;
    let supplied = authorization.as_deref().and_then(bearer_value);
    match token {
        None => Ok(Some(
            Response::from_json(&serde_json::json!({
                "error": "processor authentication is not configured",
                "code": "processor_authentication_not_configured",
            }))?
            .with_status(500),
        )),
        Some(token) if supplied.is_some_and(|value| constant_time_equal(value, &token)) => Ok(None),
        Some(_) => {
            let mut response = Response::from_json(&serde_json::json!({
                "error": "processor authentication required",
                "code": "processor_authentication_required",
            }))?
            .with_status(401);
            response.headers_mut().set("www-authenticate", "Bearer")?;
            Ok(Some(response))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bearer_value, constant_time_equal, cookie_value, requires_auth};

    #[test]
    fn protects_api_and_media_but_not_static_assets() {
        assert!(requires_auth("/api/series"));
        assert!(requires_auth("/audio/file"));
        assert!(requires_auth("/images/file"));
        assert!(requires_auth("/epubs/file"));
        assert!(!requires_auth("/"));
        assert!(!requires_auth("/favicon.svg"));
        assert!(!requires_auth("/css/base.css"));
    }
    #[test]
    fn compares_tokens_without_prefix_shortcuts() {
        assert!(constant_time_equal("token", "token"));
        assert!(!constant_time_equal("token", "token2"));
        assert!(!constant_time_equal("token", "Token"));
    }

    #[test]
    fn parses_supported_credentials() {
        assert_eq!(bearer_value("Bearer abc"), Some("abc"));
        assert_eq!(bearer_value("bearer abc"), Some("abc"));
        assert_eq!(bearer_value("Basic abc"), None);
        assert_eq!(
            cookie_value("a=1; dantalian_api_token=abc; z=2", "dantalian_api_token"),
            Some("abc".into())
        );
        assert_eq!(
            cookie_value("dantalian_api_token=abc%20def", "dantalian_api_token"),
            Some("abc def".into())
        );
    }
}
