use crate::{
    external_api::{fetch_with_timeout, read_bytes_limited},
    wasabi::{WasabiConfig, WasabiStorage},
};
use dantalian::{
    narou::{self, Config, FavoriteTags, ImportMapping, Novel, NovelList, SyncFailure, SyncResult},
    ports::object_storage::{ObjectKind, WORKER_DIRECT_UPLOAD_MAX_BYTES, object_key},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};
use worker::{
    D1Database, D1Type, Fetch, Headers, Method, Request, RequestInit, RequestRedirect, Response,
    Result, RouteContext,
};

pub(crate) fn validate_url(value: &str) -> std::result::Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "narou.rs の URL が不正です".to_string())?;
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

pub(crate) fn validate_connection(config: &Config) -> std::result::Result<url::Url, String> {
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

async fn get(config: &Config, path: &str, limit: usize) -> std::result::Result<Vec<u8>, String> {
    let headers = Headers::new();
    if !config.token.is_empty() {
        headers
            .set("Authorization", &format!("Bearer {}", config.token))
            .map_err(|_| "API トークンが不正です".to_string())?;
        headers
            .set("x-narou-internal-token", &config.token)
            .map_err(|_| "API トークンが不正です".to_string())?;
    }
    if !config.cf_access_client_id.is_empty() {
        headers
            .set("CF-Access-Client-Id", &config.cf_access_client_id)
            .map_err(|_| "Cloudflare Client ID が不正です".to_string())?;
        headers
            .set("CF-Access-Client-Secret", &config.cf_access_client_secret)
            .map_err(|_| "Cloudflare Client Secret が不正です".to_string())?;
    }
    let mut init = RequestInit::new();
    init.with_method(Method::Get)
        .with_headers(headers)
        .with_redirect(RequestRedirect::Manual);
    let request = Request::new_with_init(&format!("{}{path}", config.url), &init)
        .map_err(|_| "narou.rs のリクエストを作成できませんでした".to_string())?;
    let response = fetch_with_timeout(Fetch::Request(request), "narou.rs")
        .await
        .map_err(|_| "narou.rs への接続に失敗しました".to_string())?;
    if !(200..300).contains(&response.status_code()) {
        return Err(format!(
            "narou.rs が HTTP {} を返しました",
            response.status_code()
        ));
    }
    read_bytes_limited(response, "narou.rs", limit)
        .await
        .map_err(|_| "narou.rs の応答を受信できませんでした（サイズ上限を確認してください）".into())
}

fn api_error(status: u16, message: impl Into<String>) -> Result<Response> {
    Response::from_json(&serde_json::json!({"error":message.into()}))
        .map(|response| response.with_status(status))
}

#[derive(Deserialize)]
struct SettingRow {
    key: String,
    value: String,
}

pub(crate) async fn load_settings(db: &D1Database) -> Result<HashMap<String, String>> {
    let rows = db
        .prepare("SELECT key,value FROM settings WHERE key LIKE 'narou.%'")
        .all()
        .await?
        .results::<SettingRow>()?;
    Ok(rows.into_iter().map(|row| (row.key, row.value)).collect())
}

pub async fn sync(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let db = ctx.d1("DB")?;
    let settings = load_settings(&db).await?;
    if settings
        .get(narou::KEY_URL)
        .is_none_or(|url| url.trim().is_empty())
    {
        reconcile(&db, "", &[]).await?;
        return Response::from_json(&SyncResult::default());
    }
    let mut config = match Config::from_settings(&settings) {
        Ok(config) => config,
        Err(message) => return api_error(400, message),
    };
    config.url = match validate_connection(&config) {
        Ok(url) => url.as_str().trim_end_matches('/').to_owned(),
        Err(message) => return api_error(400, message),
    };
    let favorites = if config.use_favorites {
        let bytes = match get(&config, "/api/favorite_tags", 1024 * 1024).await {
            Ok(bytes) => bytes,
            Err(message) => return api_error(502, message),
        };
        let favorites: FavoriteTags = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => return api_error(502, "お気に入りタグの応答が不正です"),
        };
        if !favorites.success {
            return api_error(502, "narou.rs のお気に入りタグを取得できませんでした");
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
        reconcile(&db, &config.url, &[]).await?;
        return Response::from_json(&report);
    }
    let bytes = match get(&config, "/api/list?all=true", 16 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(message) => return api_error(502, message),
    };
    let list: NovelList = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => return api_error(502, "小説一覧の応答が不正です"),
    };
    drop(bytes);
    let mut novels = list.data;
    novels.retain(|novel| novel.matches(&selected));
    let ids: Vec<_> = novels.iter().map(|novel| novel.id).collect();
    reconcile(&db, &config.url, &ids).await?;
    if novels.is_empty() {
        return Response::from_json(&report);
    }
    let storage_config = match WasabiConfig::from_env(&ctx.env).await {
        Ok(config) => config,
        Err(_) => return api_error(500, "EPUB 保存先の Wasabi 設定を確認してください"),
    };
    let storage = WasabiStorage::new(storage_config.clone());
    let mut seen = HashSet::new();
    for mut novel in novels {
        if !seen.insert(novel.id) {
            continue;
        }
        report.matched += 1;
        let result = async {
            let id = i32::try_from(novel.id)
                .ok()
                .filter(|id| *id > 0)
                .ok_or_else(|| "小説 ID が不正です".to_string())?;
            let version = serde_json::to_string(&novel).expect("serializing novel metadata");
            let cached = db
                .prepare(narou::CACHE_SQL)
                .bind_refs([&D1Type::Text(&config.url), &D1Type::Integer(id)])
                .map_err(|_| "同期履歴を読み込めませんでした".to_string())?
                .first::<narou::CachedNovel>(None)
                .await
                .map_err(|_| "同期履歴を読み込めませんでした".to_string())?;
            if let Some(cached) = cached {
                if cached.remote_version == version {
                    return Ok((cached.book_id, None));
                }
            }
            let bytes = get(
                &config,
                &format!("/novels/{id}/download"),
                WORKER_DIRECT_UPLOAD_MAX_BYTES as usize,
            )
            .await?;
            narou::validate_epub(&bytes)?;
            let mut hash = String::with_capacity(64);
            for byte in Sha256::digest(&bytes) {
                write!(&mut hash, "{byte:02x}").expect("writing a string");
            }
            let file_hash = format!("{hash}.epub");
            let filename = format!("{}.epub", novel.title);
            let key = object_key(
                storage_config.prefix.as_deref(),
                ObjectKind::Epub,
                &hash,
                "epub",
            )
            .map_err(|_| "EPUB 保存キーが不正です".to_string())?;
            storage
                .put_object(&key, "application/epub+zip", &bytes)
                .await
                .map_err(|_| "EPUB を Wasabi へ保存できませんでした".to_string())?;
            let mut authors = crate::author_repository::NormalizedAuthorLookup::load(
                &db,
                [novel.author.as_str()],
            )
            .await
            .map_err(|_| "著者情報を読み込めませんでした".to_string())?;
            if let Some(author) = authors.get(&novel.author) {
                novel.author = author.name.clone();
            }
            persist(
                &db,
                &config.url,
                &novel,
                id,
                &file_hash,
                &filename,
                &key,
                bytes.len(),
                &version,
            )
            .await
            .map(|(id, created)| (id, Some(created)))
            .map_err(|_| "小説をデータベースへ保存できませんでした".to_string())
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
    Response::from_json(&report)
}

async fn reconcile(db: &D1Database, source: &str, ids: &[i64]) -> Result<()> {
    let ids = serde_json::to_string(ids).expect("serializing novel IDs");
    db.prepare(narou::RECONCILE_SQL)
        .bind_refs([&D1Type::Text(source), &D1Type::Text(&ids)])?
        .run()
        .await?;
    Ok(())
}

async fn persist(
    db: &D1Database,
    source: &str,
    novel: &Novel,
    id: i32,
    hash: &str,
    filename: &str,
    key: &str,
    size: usize,
    version: &str,
) -> Result<(i64, bool)> {
    let description = novel.description();
    let values = [
        D1Type::Text(source),
        D1Type::Integer(id),
        D1Type::Text(&novel.title),
        D1Type::Text(&novel.sitename),
        D1Type::Text(&description),
        D1Type::Text(&novel.author),
        D1Type::Text(hash),
        D1Type::Text(filename),
        D1Type::Text(version),
    ];
    let mut statements = Vec::with_capacity(narou::IMPORT_SQL.len() + 3);
    statements.push(db.prepare(narou::MAPPING_SQL).bind_refs(&values[..2])?);
    for (sql, count) in narou::IMPORT_SQL {
        statements.push(db.prepare(*sql).bind_refs(&values[..*count])?);
    }
    statements.push(db.prepare("INSERT OR REPLACE INTO object_uploads(object_key,object_kind,entity_id,content_type,extension,expected_size,original_name,status,created_at) SELECT ?3,'epub',book_id,'application/epub+zip','epub',?4,?5,'complete',CURRENT_TIMESTAMP FROM narou_imports WHERE source_url=?1 AND novel_id=?2")
        .bind_refs([&values[0], &values[1], &D1Type::Text(key), &D1Type::Integer(size as i32), &D1Type::Text(filename)])?);
    statements.push(db.prepare(narou::MAPPING_SQL).bind_refs(&values[..2])?);
    let results = db.batch(statements).await?;
    let created = results[0].results::<ImportMapping>()?.is_empty();
    let stored = results
        .last()
        .ok_or_else(|| worker::Error::from("missing import result"))?
        .results::<ImportMapping>()?;
    let mapping = stored
        .first()
        .ok_or_else(|| worker::Error::from("missing imported novel"))?;
    Ok((mapping.book_id, created))
}
