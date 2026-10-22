//! 全レスポンスに付与するセキュリティヘッダー。
//!
//! Content-Security-Policy はインラインイベントハンドラと Bibi リーダーの
//! 動作確認が必要なためここでは付与しない（README の残作業を参照）。

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

const SECURITY_HEADERS: [(&str, &str); 5] = [
    ("x-content-type-options", "nosniff"),
    ("x-frame-options", "SAMEORIGIN"),
    ("referrer-policy", "same-origin"),
    (
        "permissions-policy",
        "camera=(), microphone=(), geolocation=()",
    ),
    // ホスト UI はインラインイベントハンドラを、Bibi は DOMPurify 済み EPUB の
    // インラインスタイル・new Function を使うため inline/eval は許可する。
    // それ以外は同一 origin に限定し、object/base/form とフレーム外埋め込みを塞ぐ。
    (
        "content-security-policy",
        "default-src 'self'; \
         script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'; \
         style-src 'self' 'unsafe-inline' blob:; \
         img-src 'self' data: blob: https:; \
         media-src 'self' data: blob: https:; \
         font-src 'self' data: blob: https:; \
         frame-src 'self' blob:; \
         worker-src 'self' blob:; \
         child-src 'self' blob:; \
         connect-src 'self'; \
         object-src 'none'; \
         base-uri 'none'; \
         form-action 'self'; \
         frame-ancestors 'self'",
    ),
];

pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in SECURITY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}
