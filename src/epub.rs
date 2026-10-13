use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::{self, Next},
    response::Response,
};
use std::{path::Component, sync::Arc};
use tower_http::services::ServeDir;

pub fn routes(directory: Arc<String>) -> Router {
    Router::new()
        .fallback_service(ServeDir::new(directory.as_ref()))
        .layer(middleware::from_fn_with_state(
            directory,
            normalize_suffix_range,
        ))
}

async fn normalize_suffix_range(
    State(directory): State<Arc<String>>,
    mut request: Request,
    next: Next,
) -> Response {
    let suffix_length = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes=-"))
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<u64>().ok());

    if let Some(suffix_length) = suffix_length.filter(|length| *length > 0) {
        if let Ok(path) = urlencoding::decode(request.uri().path()) {
            let path = path.trim_start_matches('/');
            // Apply the same directory boundary before looking up range metadata.
            if !path.contains(['\\', ':', '\0'])
                && std::path::Path::new(path)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
            {
                let file = std::path::Path::new(directory.as_ref()).join(path);
                if let Ok(metadata) = tokio::fs::metadata(file).await {
                    if metadata.is_file() && metadata.len() > 0 && suffix_length > metadata.len() {
                        // RFC 9110: a suffix longer than the representation covers the whole file.
                        request
                            .headers_mut()
                            .insert(header::RANGE, HeaderValue::from_static("bytes=0-"));
                    }
                }
            }
        }
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::StatusCode};
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    use tower::ServiceExt;

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "dantalian-epub-range-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("small epub"), b"small EPUB").unwrap();
            Self(path)
        }

        fn app(&self) -> Router {
            Router::new().nest(
                "/epubs",
                routes(Arc::new(self.0.to_str().unwrap().to_owned())),
            )
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    async fn response(directory: &TestDirectory, range: &str) -> Response {
        directory
            .app()
            .oneshot(
                Request::builder()
                    .uri("/epubs/small%20epub")
                    .header(header::RANGE, range)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn oversized_suffix_serves_the_whole_nonempty_file() {
        let directory = TestDirectory::new();
        let result = response(&directory, "bytes=-65557").await;
        assert_eq!(result.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(result.headers()[header::CONTENT_RANGE], "bytes 0-9/10");
        assert_eq!(
            axum::body::to_bytes(result.into_body(), 10).await.unwrap(),
            &b"small EPUB"[..]
        );
    }

    #[tokio::test]
    async fn ordinary_suffix_remains_a_partial_response() {
        let directory = TestDirectory::new();
        let result = response(&directory, "bytes=-4").await;
        assert_eq!(result.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(result.headers()[header::CONTENT_RANGE], "bytes 6-9/10");
        assert_eq!(
            axum::body::to_bytes(result.into_body(), 4).await.unwrap(),
            &b"EPUB"[..]
        );
    }
}
