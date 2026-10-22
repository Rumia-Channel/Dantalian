//! Native サーバーの認証。
//!
//! Worker (`worker/src/auth.rs`) と同じ資格情報形式
//! (`Authorization: Bearer` または `dantalian_api_token` Cookie) を受け付ける。
//! トークンが設定されていない場合のみ認証を無効にする（既定はループバック
//! バインドなのでローカル利用はそのまま動く）。非ループバックで待ち受ける
//! 場合は `main.rs` がトークン未設定の起動を拒否する。

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

pub const TOKEN_ENV: &str = "DANTALIAN_API_TOKEN";
pub const COOKIE_NAME: &str = "dantalian_api_token";

#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthDecision {
    Allow,
    Reject,
}

impl AuthConfig {
    pub fn new(token: Option<String>) -> Self {
        let token = token
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Self { token }
    }

    /// 環境変数 `DANTALIAN_API_TOKEN` から設定を読む。
    pub fn from_env() -> Self {
        Self::new(std::env::var(TOKEN_ENV).ok())
    }

    /// 認証が必須かどうか。トークンが設定されていれば必須。
    pub fn required(&self) -> bool {
        self.token.is_some()
    }

    fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }
}

/// 認証が必要なパスか。静的アセットはログイン画面を表示するため公開する。
pub fn requires_auth(path: &str) -> bool {
    path.starts_with("/api/")
        || path.starts_with("/audio/")
        || path.starts_with("/images/")
        || path.starts_with("/epubs/")
}

/// `/api/health` は認証設定を問い合わせるための公開エンドポイント。
fn is_public_api_path(path: &str) -> bool {
    path == "/api/health"
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

fn supplied_credential(request: &Request) -> Option<String> {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bearer_value)
        .map(str::to_owned)
        .or_else(|| {
            request
                .headers()
                .get(header::COOKIE)
                .and_then(|value| value.to_str().ok())
                .and_then(|header| cookie_value(header, COOKIE_NAME))
        })
}

fn decision(config: &AuthConfig, request: &Request) -> AuthDecision {
    let Some(token) = config.token() else {
        return AuthDecision::Allow;
    };
    match supplied_credential(request) {
        Some(supplied) if constant_time_equal(&supplied, token) => AuthDecision::Allow,
        _ => AuthDecision::Reject,
    }
}

fn unauthorized() -> Response {
    let mut response = (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({
            "error": "authentication required",
            "code": "authentication_required",
        })),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

/// `/api`・メディア配信パスにトークン認証を要求するミドルウェア。
pub async fn require_auth(
    State(config): State<AuthConfig>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if !requires_auth(path) || is_public_api_path(path) {
        return next.run(request).await;
    }
    match decision(&config, &request) {
        AuthDecision::Allow => next.run(request).await,
        AuthDecision::Reject => unauthorized(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthConfig, COOKIE_NAME, bearer_value, constant_time_equal, cookie_value, requires_auth,
    };

    #[test]
    fn protects_api_and_media_but_not_static_assets() {
        assert!(requires_auth("/api/books"));
        assert!(requires_auth("/audio/file"));
        assert!(requires_auth("/images/file"));
        assert!(requires_auth("/epubs/file"));
        assert!(!requires_auth("/"));
        assert!(!requires_auth("/books/"));
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
            cookie_value("a=1; dantalian_api_token=abc; z=2", COOKIE_NAME),
            Some("abc".into())
        );
        assert_eq!(
            cookie_value("dantalian_api_token=abc%20def", COOKIE_NAME),
            Some("abc def".into())
        );
    }

    #[test]
    fn authentication_is_optional_without_a_token() {
        let config = AuthConfig::new(None);
        assert!(!config.required());
        assert!(!AuthConfig::new(Some("   ".to_string())).required());
        assert!(AuthConfig::new(Some("secret".to_string())).required());
    }
}
