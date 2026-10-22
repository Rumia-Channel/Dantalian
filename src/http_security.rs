//! 全レスポンスに付与するセキュリティヘッダーと、バージョン付き静的
//! アセットの長期キャッシュ。

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, header},
    middleware::Next,
    response::Response,
};

/// `?v=<ASSET_VERSION>` で参照される静的アセットは内容が変わると URL も
/// 変わるため、長期間キャッシュしてよい。Bibi の HTML エントリ点は
/// バージョン付き URLではないので対象外にする。
fn is_versioned_asset(path: &str) -> bool {
    const DIRECT_ASSET_PREFIXES: [&str; 4] = ["/js/", "/css/", "/fonts/", "/wasm/"];
    const BIBI_ASSET_PREFIXES: [&str; 5] = [
        "/vendor/bibi/and/",
        "/vendor/bibi/extensions/",
        "/vendor/bibi/presets/",
        "/vendor/bibi/resources/",
        "/vendor/bibi/wardrobe/",
    ];
    DIRECT_ASSET_PREFIXES
        .iter()
        .chain(BIBI_ASSET_PREFIXES.iter())
        .any(|prefix| path.starts_with(prefix))
}

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
    let versioned_asset = is_versioned_asset(request.uri().path());
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in SECURITY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    if versioned_asset {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}
