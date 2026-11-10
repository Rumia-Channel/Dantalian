use crate::{
    AppState,
    narou::{Config, FavoriteTags, NovelList, SyncFailure, SyncResult},
};
use axum::{Json, extract::State, http::StatusCode};
use std::{collections::HashSet, time::Duration};

type ApiError = (StatusCode, Json<serde_json::Value>);
fn error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(serde_json::json!({"error":message.into()})))
}

pub(super) fn validate_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "narou.rs の URL が不正です".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "narou.rs URL は認証情報・クエリ・フラグメントを含まない HTTP(S) URL にしてください"
                .into(),
        );
    }
    Ok(url)
}

pub(super) fn validate_connection(config: &Config) -> Result<reqwest::Url, String> {
    let url = validate_url(&config.url)?;
    if config.has_credentials()
        && url.scheme() != "https"
        && !matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        )
    {
        return Err(
            "認証情報を送信する narou.rs 接続には HTTPS が必要です（ループバックのみ HTTP 可）"
                .into(),
        );
    }
    Ok(url)
}

async fn get(
    client: &reqwest::Client,
    config: &Config,
    path: &str,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut request = client.get(format!("{}{path}", config.url));
    if !config.token.is_empty() {
        request = request
            .bearer_auth(&config.token)
            .header("x-narou-internal-token", &config.token);
    }
    if !config.cf_access_client_id.is_empty() {
        request = request
            .header("CF-Access-Client-Id", &config.cf_access_client_id)
            .header("CF-Access-Client-Secret", &config.cf_access_client_secret);
    }
    let mut response = request
        .send()
        .await
        .map_err(|_| "narou.rs への接続に失敗しました".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "narou.rs が HTTP {} を返しました",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("narou.rs の応答がサイズ上限を超えています".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "narou.rs の応答を受信できませんでした".to_string())?
    {
        if chunk.len() > limit - bytes.len() {
            return Err("narou.rs の応答がサイズ上限を超えています".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub async fn sync(State(state): State<AppState>) -> Result<Json<SyncResult>, ApiError> {
    let settings = state.db.get_all_settings();
    if settings
        .get(crate::narou::KEY_URL)
        .is_none_or(|url| url.trim().is_empty())
    {
        state.db.reconcile_narou_selection("", &[]).map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "同期対象を更新できませんでした",
            )
        })?;
        return Ok(Json(SyncResult::default()));
    }
    let mut config = Config::from_settings(&settings)
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    config.url = validate_connection(&config)
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?
        .as_str()
        .trim_end_matches('/')
        .to_owned();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HTTP クライアントを作成できませんでした",
            )
        })?;
    let favorites = if config.use_favorites {
        let bytes = get(&client, &config, "/api/favorite_tags", 1024 * 1024)
            .await
            .map_err(|message| error(StatusCode::BAD_GATEWAY, message))?;
        let favorites: FavoriteTags = serde_json::from_slice(&bytes)
            .map_err(|_| error(StatusCode::BAD_GATEWAY, "お気に入りタグの応答が不正です"))?;
        if !favorites.success {
            return Err(error(
                StatusCode::BAD_GATEWAY,
                "narou.rs のお気に入りタグを取得できませんでした",
            ));
        }
        favorites.tags
    } else {
        Vec::new()
    };
    let selected = config.selected_tags(&favorites);
    let mut report = SyncResult {
        tags: selected.iter().cloned().collect(),
        ..Default::default()
    };
    if selected.is_empty() {
        state
            .db
            .reconcile_narou_selection(&config.url, &[])
            .map_err(|_| {
                error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "同期対象を更新できませんでした",
                )
            })?;
        return Ok(Json(report));
    }
    let bytes = get(&client, &config, "/api/list?all=true", 16 * 1024 * 1024)
        .await
        .map_err(|message| error(StatusCode::BAD_GATEWAY, message))?;
    let list: NovelList = serde_json::from_slice(&bytes)
        .map_err(|_| error(StatusCode::BAD_GATEWAY, "小説一覧の応答が不正です"))?;
    drop(bytes);
    let mut novels = list.data;
    novels.retain(|novel| novel.matches(&selected));
    let ids: Vec<_> = novels.iter().map(|novel| novel.id).collect();
    state
        .db
        .reconcile_narou_selection(&config.url, &ids)
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "同期対象を更新できませんでした",
            )
        })?;
    let limit = super::upload_limit_bytes(&state, super::KEY_UPLOAD_FILE_MB, super::EPUB_MAX_BYTES);
    let mut seen = HashSet::new();
    for novel in novels {
        if !seen.insert(novel.id) {
            continue;
        }
        report.matched += 1;
        let result = async {
            if novel.id <= 0 {
                return Err("小説 ID が不正です".to_string());
            }
            let version = serde_json::to_string(&novel).expect("serializing novel metadata");
            if let Some(cached) = state
                .db
                .cached_narou_novel(&config.url, novel.id)
                .map_err(|_| "同期履歴を読み込めませんでした".to_string())?
            {
                if cached.remote_version == version
                    && std::path::Path::new(state.epubs_dir.as_str())
                        .join(&cached.file_hash)
                        .is_file()
                {
                    return Ok((cached.book_id, None));
                }
            }
            let bytes = get(
                &client,
                &config,
                &format!("/novels/{}/download", novel.id),
                limit,
            )
            .await?;
            crate::narou::validate_epub(&bytes)?;
            let filename = format!("{}.epub", novel.title);
            let (hash, _) =
                crate::external::save_uploaded_file(&bytes, &filename, &state.epubs_dir, limit)?;
            // The transaction owns only the imported attachment; other files and reading state survive refresh.
            let stored =
                state
                    .db
                    .import_narou_novel(&config.url, &novel, &hash, &filename, &version);
            match stored {
                Ok((id, created, _)) => Ok((id, Some(created))),
                Err(_) => Err("小説をデータベースへ保存できませんでした".to_string()),
            }
        }
        .await;
        match result {
            Ok((id, created)) => {
                report.book_ids.push(id);
                match created {
                    Some(true) => report.imported += 1,
                    Some(false) => report.updated += 1,
                    None => report.unchanged += 1,
                }
            }
            Err(message) => report.failed.push(SyncFailure {
                novel_id: novel.id,
                title: novel.title,
                error: message,
            }),
        }
    }
    Ok(Json(report))
}
