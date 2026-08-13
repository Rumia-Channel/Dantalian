use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use aws_sdk_s3::{
    config::{Credentials, Region},
    primitives::ByteStream,
};
use reqwest::Client;
use rusqlite::{Connection, OptionalExtension, params, types::Value};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sha3::{Digest, Sha3_256};

const DEFAULT_STATE_PATH: &str = "dantalian-migration-state.json";
const DEFAULT_REPORT_PATH: &str = "dantalian-migration-report.json";
const TABLES: &[&str] = &[
    "series",
    "grand_series",
    "grand_series_items",
    "authors",
    "storage_locations",
    "labels",
    "books",
    "book_authors",
    "copies",
    "borrowers",
    "lending_history",
    "settings",
    "cds",
    "tracks",
    "playlists",
    "playlist_tracks",
    "cd_authors",
    "track_metadata",
    "cd_metadata",
    "track_authors",
];

#[derive(Debug)]
struct Args {
    sqlite: PathBuf,
    media_root: PathBuf,
    apply: bool,
    state_path: PathBuf,
    report_path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MigrationState {
    completed_rows: BTreeSet<String>,
    uploaded_objects: BTreeSet<String>,
}

#[derive(Debug, Serialize)]
struct MigrationReport {
    source: String,
    media_root: String,
    mode: &'static str,
    tables: BTreeMap<String, TableReport>,
    media: Vec<MediaReport>,
    reconciliation: Option<ReconciliationReport>,
}

#[derive(Debug, Serialize)]
struct TableReport {
    source_rows: usize,
    planned_rows: usize,
    applied_rows: usize,
    skipped_rows: usize,
}

#[derive(Debug, Serialize)]
struct MediaReport {
    object_key: String,
    source_path: String,
    size_bytes: u64,
    sha3_256: String,
    uploaded: bool,
}

#[derive(Debug, Serialize)]
struct ReconciliationReport {
    source_counts: BTreeMap<String, usize>,
    destination_counts: BTreeMap<String, usize>,
    missing_counts: BTreeMap<String, usize>,
    extra_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct MediaObject {
    source_path: PathBuf,
    object_key: String,
    content_type: String,
    size_bytes: u64,
    sha3_256: String,
}

#[derive(Debug, Serialize)]
struct D1Query {
    sql: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    params: Vec<JsonValue>,
}

#[derive(Debug, Deserialize)]
struct D1Response {
    success: bool,
}

#[derive(Clone)]
struct D1Client {
    client: Client,
    endpoint: String,
    api_token: String,
}

struct WasabiUploader {
    client: aws_sdk_s3::Client,
    bucket: String,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    run(Args::parse(env::args().skip(1))?).await
}

async fn run(args: Args) -> Result<(), String> {
    let source = Connection::open(&args.sqlite)
        .map_err(|_| "could not open the SQLite source database".to_string())?;
    source
        .execute_batch("PRAGMA foreign_keys = ON")
        .map_err(|_| "could not enable SQLite foreign keys".to_string())?;
    let mut state = load_state(&args.state_path)?;
    let mut report = MigrationReport {
        source: args.sqlite.display().to_string(),
        media_root: args.media_root.display().to_string(),
        mode: if args.apply { "apply" } else { "dry-run" },
        tables: BTreeMap::new(),
        media: Vec::new(),
        reconciliation: None,
    };

    let d1 = if args.apply {
        Some(D1Client::from_env()?)
    } else {
        None
    };
    for table in TABLES {
        let rows = read_table(&source, table)?;
        let source_rows = rows.len();
        let mut table_report = TableReport {
            source_rows,
            planned_rows: source_rows,
            applied_rows: 0,
            skipped_rows: 0,
        };
        if let Some(d1) = d1.as_ref() {
            for chunk in rows.chunks(50) {
                let mut queries = Vec::with_capacity(chunk.len());
                let mut keys = Vec::with_capacity(chunk.len());
                for row in chunk {
                    let key = migration_key(table, row);
                    if state.completed_rows.contains(&key) {
                        table_report.skipped_rows += 1;
                        continue;
                    }
                    queries.push(row.destination_row(table).insert_query(table)?);
                    keys.push(key);
                }
                if queries.is_empty() {
                    continue;
                }
                d1.execute_batch(&queries)
                    .await
                    .map_err(|_| format!("D1 migration failed while applying table {table}"))?;
                for key in keys {
                    state.completed_rows.insert(key);
                }
                table_report.applied_rows += queries.len();
                save_state(&args.state_path, &state)?;
            }
        }
        report.tables.insert((*table).to_string(), table_report);
    }

    let media = discover_media(
        &source,
        &args.media_root,
        optional_env("WASABI_PREFIX").as_deref(),
    )?;
    let uploader = if args.apply {
        Some(WasabiUploader::from_env()?)
    } else {
        None
    };
    for object in &media {
        let uploaded = if let Some(uploader) = uploader.as_ref() {
            let key = object.object_key.clone();
            if state.uploaded_objects.contains(&key) {
                false
            } else {
                uploader.upload(object).await?;
                state.uploaded_objects.insert(key);
                save_state(&args.state_path, &state)?;
                true
            }
        } else {
            false
        };
        report.media.push(MediaReport {
            object_key: object.object_key.clone(),
            source_path: object.source_path.display().to_string(),
            size_bytes: object.size_bytes,
            sha3_256: object.sha3_256.clone(),
            uploaded,
        });
    }

    if let Some(d1) = d1.as_ref() {
        let metadata = media
            .iter()
            .map(|object| media_metadata_queries(&source, object))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        for chunk in metadata.chunks(50) {
            d1.execute_batch(chunk).await?;
        }
        report.reconciliation = Some(reconcile(&source, d1).await?);
    }
    write_json(&args.report_path, &report)?;
    println!(
        "migration {}: {} tables, {} media objects; report written to {}",
        report.mode,
        report.tables.len(),
        report.media.len(),
        args.report_path.display()
    );
    Ok(())
}

impl Args {
    fn parse<I>(mut values: I) -> Result<Self, String>
    where
        I: Iterator<Item = String>,
    {
        let mut sqlite = None;
        let mut media_root = None;
        let mut apply = false;
        let mut state_path = PathBuf::from(DEFAULT_STATE_PATH);
        let mut report_path = PathBuf::from(DEFAULT_REPORT_PATH);
        while let Some(value) = values.next() {
            match value.as_str() {
                "--sqlite" => sqlite = Some(PathBuf::from(next_arg(&mut values, "--sqlite")?)),
                "--media-root" => {
                    media_root = Some(PathBuf::from(next_arg(&mut values, "--media-root")?))
                }
                "--apply" => apply = true,
                "--state" => state_path = PathBuf::from(next_arg(&mut values, "--state")?),
                "--report" => report_path = PathBuf::from(next_arg(&mut values, "--report")?),
                "--help" | "-h" => return Err(usage().to_string()),
                unknown => return Err(format!("unknown migration option: {unknown}\n{}", usage())),
            }
        }
        Ok(Self {
            sqlite: sqlite.ok_or_else(|| format!("--sqlite is required\n{}", usage()))?,
            media_root: media_root
                .ok_or_else(|| format!("--media-root is required\n{}", usage()))?,
            apply,
            state_path,
            report_path,
        })
    }
}

fn usage() -> &'static str {
    "usage: dantalian_migrate --sqlite PATH --media-root PATH [--apply] [--state PATH] [--report PATH]"
}

fn next_arg<I>(values: &mut I, option: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    values
        .next()
        .ok_or_else(|| format!("{option} requires a value\n{}", usage()))
}

fn read_table(connection: &Connection, table: &str) -> Result<Vec<SqlRow>, String> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM \"{table}\""))
        .map_err(|_| format!("could not read SQLite table {table}"))?;
    let columns = statement
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let rows = statement
        .query_map([], |row| {
            let values = (0..columns.len())
                .map(|index| row.get::<_, Value>(index))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(SqlRow {
                columns: columns.clone(),
                values,
            })
        })
        .map_err(|_| format!("could not enumerate SQLite table {table}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("could not decode SQLite table {table}"))?;
    Ok(rows)
}

#[derive(Debug)]
struct SqlRow {
    columns: Vec<String>,
    values: Vec<Value>,
}

impl SqlRow {
    fn insert_query(&self, table: &str) -> Result<D1Query, String> {
        let mut params = Vec::new();
        let mut placeholders = Vec::with_capacity(self.values.len());
        for value in &self.values {
            match value {
                Value::Null => placeholders.push("NULL".to_string()),
                Value::Blob(bytes) => placeholders.push(format!("X'{}'", hex(bytes))),
                _ => {
                    params.push(sql_value(value)?);
                    placeholders.push("?".to_string());
                }
            }
        }
        let columns = self
            .columns
            .iter()
            .map(|column| format!("\"{column}\""))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(D1Query {
            sql: format!(
                "INSERT OR IGNORE INTO \"{table}\" ({columns}) VALUES ({})",
                placeholders.join(", ")
            ),
            params,
        })
    }

    fn destination_row(&self, table: &str) -> Self {
        let mut row = Self {
            columns: self.columns.clone(),
            values: self.values.clone(),
        };
        let hash_column = match table {
            "books" => "epub_file_hash",
            "tracks" => "file_hash",
            _ => return row,
        };
        let Some(hash_index) = row.columns.iter().position(|column| column == hash_column) else {
            return row;
        };
        let Some(Value::Text(hash)) = row.values.get(hash_index) else {
            return row;
        };
        let hash = hash.clone();
        if hash.is_empty() {
            return row;
        }
        if table == "tracks" {
            let object_id = hash.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(&hash);
            row.values[hash_index] = Value::Text(object_id.to_string());
            return row;
        }
        if hash.contains('.') {
            return row;
        }
        let name_column = if table == "books" {
            "epub_file_name"
        } else {
            "file_name"
        };
        let extension = row
            .columns
            .iter()
            .position(|column| column == name_column)
            .and_then(|index| match &row.values[index] {
                Value::Text(name) => Path::new(name).extension(),
                _ => None,
            })
            .and_then(|extension| extension.to_str())
            .filter(|extension| !extension.is_empty())
            .unwrap_or(if table == "books" { "epub" } else { "bin" });
        row.values[hash_index] = Value::Text(format!("{hash}.{extension}"));
        row
    }
}

fn sql_value(value: &Value) -> Result<JsonValue, String> {
    Ok(match value {
        Value::Null => JsonValue::Null,
        Value::Integer(value) => json!(value),
        Value::Real(value) => json!(value),
        Value::Text(value) => json!(value),
        Value::Blob(_) => return Err("blob values must be inlined as hexadecimal".to_string()),
    })
}

fn migration_key(table: &str, row: &SqlRow) -> String {
    let mut key = table.to_string();
    for column in [
        "id",
        "book_id",
        "cd_id",
        "track_id",
        "playlist_id",
        "author_id",
    ] {
        if let Some(index) = row.columns.iter().position(|value| value == column) {
            key.push('|');
            key.push_str(column);
            key.push('=');
            key.push_str(&format_value(&row.values[index]));
        }
    }
    if key == table {
        key.push('|');
        key.push_str(
            &row.values
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    key
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Integer(value) => value.to_string(),
        Value::Real(value) => value.to_string(),
        Value::Text(value) => value.clone(),
        Value::Blob(value) => hex(value),
    }
}

fn discover_media(
    connection: &Connection,
    root: &Path,
    object_prefix: Option<&str>,
) -> Result<Vec<MediaObject>, String> {
    let mut objects = BTreeMap::<String, (PathBuf, String)>::new();
    add_media_rows(
        connection,
        root,
        &mut objects,
        object_prefix,
        "books",
        "cover_url",
        "images",
    )?;
    add_media_rows(
        connection,
        root,
        &mut objects,
        object_prefix,
        "cds",
        "cover_url",
        "images",
    )?;
    add_media_rows(
        connection,
        root,
        &mut objects,
        object_prefix,
        "books",
        "epub_file_hash",
        "epubs",
    )?;
    add_audio_rows(connection, root, &mut objects, object_prefix)?;
    let mut result = Vec::new();
    for (object_key, (source_path, content_type)) in objects {
        if !source_path.is_file() {
            return Err(format!(
                "media source is referenced by SQLite but missing: {}",
                source_path.display()
            ));
        }
        let (size_bytes, sha3_256) = hash_file(&source_path)?;
        result.push(MediaObject {
            source_path,
            object_key,
            content_type,
            size_bytes,
            sha3_256,
        });
    }
    Ok(result)
}

fn add_media_rows(
    connection: &Connection,
    root: &Path,
    objects: &mut BTreeMap<String, (PathBuf, String)>,
    object_prefix: Option<&str>,
    table: &str,
    column: &str,
    directory: &str,
) -> Result<(), String> {
    if table == "books" && column == "epub_file_hash" {
        let mut statement = connection
            .prepare(
                "SELECT epub_file_hash, epub_file_name
                 FROM books
                 WHERE epub_file_hash IS NOT NULL AND epub_file_hash <> ''",
            )
            .map_err(|_| "could not inspect books.epub_file_hash".to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(|_| "could not enumerate books.epub_file_hash".to_string())?;
        for row in rows {
            let (hash, original_name) =
                row.map_err(|_| "could not read books.epub_file_hash".to_string())?;
            let extension = original_name
                .as_deref()
                .and_then(|name| Path::new(name).extension())
                .and_then(|value| value.to_str())
                .unwrap_or("epub")
                .to_ascii_lowercase();
            let file_name = normalized_media_name(&hash, &extension);
            let extension = Path::new(&file_name)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("epub")
                .to_ascii_lowercase();
            let object_key = object_key(object_prefix, &format!("{directory}/{file_name}"))?;
            objects.entry(object_key).or_insert((
                root.join(directory).join(&hash),
                content_type_for_extension(&extension),
            ));
        }
        return Ok(());
    }

    let sql = format!(
        "SELECT {column} FROM {table}
         WHERE {column} IS NOT NULL AND {column} <> ''"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| format!("could not inspect {table}.{column}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| format!("could not enumerate {table}.{column}"))?;
    for row in rows {
        let name = row.map_err(|_| format!("could not read {table}.{column}"))?;
        if name.starts_with("http://") || name.starts_with("https://") {
            continue;
        }
        let file_name = Path::new(&name)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid media filename in {table}.{column}"))?;
        let extension = Path::new(file_name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin")
            .to_ascii_lowercase();
        let object_key = object_key(object_prefix, &format!("{directory}/{file_name}"))?;
        objects.entry(object_key).or_insert((
            root.join(directory).join(file_name),
            content_type_for_extension(&extension),
        ));
    }
    Ok(())
}

fn add_audio_rows(
    connection: &Connection,
    root: &Path,
    objects: &mut BTreeMap<String, (PathBuf, String)>,
    object_prefix: Option<&str>,
) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT file_hash, file_name
             FROM tracks
             WHERE file_hash IS NOT NULL AND file_hash <> ''",
        )
        .map_err(|_| "could not inspect tracks audio references".to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|_| "could not enumerate tracks audio references".to_string())?;
    for row in rows {
        let (hash, file_name) =
            row.map_err(|_| "could not read tracks audio references".to_string())?;
        let extension = file_name
            .as_deref()
            .and_then(|name| Path::new(name).extension())
            .and_then(|value| value.to_str())
            .unwrap_or("bin")
            .to_ascii_lowercase();
        let original_name = normalized_media_name(&hash, &extension);
        let original_key = object_key(object_prefix, &format!("audio/{original_name}"))?;
        objects.entry(original_key).or_insert((
            root.join("audio").join(&hash),
            content_type_for_extension(
                Path::new(&original_name)
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("bin"),
            ),
        ));
        let hash_stem = hash.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(&hash);
        for (encoded, content_type) in [("opus", "audio/ogg"), ("aac", "audio/aac")] {
            let encoded_name = format!("{hash_stem}.{encoded}");
            let encoded_path = root
                .join("audio")
                .join("encoded")
                .join(encoded)
                .join(&encoded_name);
            if encoded_path.is_file() {
                let key = object_key(
                    object_prefix,
                    &format!("audio/encoded/{encoded}/{encoded_name}"),
                )?;
                objects
                    .entry(key)
                    .or_insert((encoded_path, content_type.to_string()));
            }
        }
    }
    Ok(())
}

fn object_key(prefix: Option<&str>, path: &str) -> Result<String, String> {
    let valid = |value: &str| {
        !value.is_empty()
            && value != "."
            && value != ".."
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    let components = path.split('/');
    if components.clone().any(|component| !valid(component)) {
        return Err(format!("invalid object path: {path}"));
    }
    let path = path.to_string();
    match prefix.map(str::trim).filter(|value| !value.is_empty()) {
        Some(prefix)
            if prefix.trim_matches('/').split('/').all(valid)
                && !prefix.starts_with('/')
                && !prefix.ends_with('/') =>
        {
            Ok(format!("{}/{}", prefix.trim_matches('/'), path))
        }
        Some(_) => Err("invalid WASABI_PREFIX".to_string()),
        None => Ok(path),
    }
}
fn normalized_media_name(object_id: &str, extension: &str) -> String {
    if Path::new(object_id).extension().is_some() {
        object_id.to_string()
    } else {
        format!("{object_id}.{extension}")
    }
}

fn content_type_for_extension(extension: &str) -> String {
    match extension {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "epub" => "application/epub+zip",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" | "opus" => "audio/ogg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn hash_file(path: &Path) -> Result<(u64, String), String> {
    let file = File::open(path).map_err(|_| format!("could not open media {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha3_256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| format!("could not read media {}", path.display()))?;
        if count == 0 {
            break;
        }
        size += count as u64;
        hasher.update(&buffer[..count]);
    }
    Ok((size, hex(&hasher.finalize())))
}

impl D1Client {
    fn from_env() -> Result<Self, String> {
        let account = required_env("CLOUDFLARE_ACCOUNT_ID")?;
        let database = required_env("CLOUDFLARE_D1_DATABASE_ID")?;
        Ok(Self {
            client: Client::new(),
            endpoint: format!(
                "https://api.cloudflare.com/client/v4/accounts/{account}/d1/database/{database}/query"
            ),
            api_token: required_env("CLOUDFLARE_API_TOKEN")?,
        })
    }

    async fn execute_batch(&self, queries: &[D1Query]) -> Result<(), String> {
        let body = json!({ "batch": queries });
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_token)
            .json(&body)
            .send()
            .await
            .map_err(|_| "D1 request failed".to_string())?;
        let status = response.status();
        let parsed = response
            .json::<D1Response>()
            .await
            .map_err(|_| "D1 returned an invalid response".to_string())?;
        if !status.is_success() || !parsed.success {
            return Err("D1 rejected the migration batch".to_string());
        }
        Ok(())
    }

    async fn count(&self, table: &str) -> Result<usize, String> {
        let query = D1Query {
            sql: format!("SELECT COUNT(*) AS count FROM \"{table}\""),
            params: Vec::new(),
        };
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_token)
            .json(&query)
            .send()
            .await
            .map_err(|_| "D1 reconciliation request failed".to_string())?;
        let body = response
            .json::<JsonValue>()
            .await
            .map_err(|_| "D1 returned an invalid reconciliation response".to_string())?;
        body["result"][0]["results"][0]["count"]
            .as_u64()
            .map(|value| value as usize)
            .ok_or_else(|| "D1 reconciliation response omitted count".to_string())
    }
}

impl WasabiUploader {
    fn from_env() -> Result<Self, String> {
        let credentials = Credentials::new(
            required_env("WASABI_ACCESS_KEY_ID")?,
            required_env("WASABI_SECRET_ACCESS_KEY")?,
            None,
            None,
            "dantalian-migration",
        );
        let config = aws_sdk_s3::Config::builder()
            .endpoint_url(required_env("WASABI_ENDPOINT")?)
            .region(Region::new(required_env("WASABI_REGION")?))
            .credentials_provider(credentials)
            .force_path_style(true)
            .build();
        Ok(Self {
            client: aws_sdk_s3::Client::from_conf(config),
            bucket: required_env("WASABI_BUCKET")?,
        })
    }

    async fn upload(&self, object: &MediaObject) -> Result<(), String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&object.object_key)
            .content_type(&object.content_type)
            .body(
                ByteStream::from_path(&object.source_path)
                    .await
                    .map_err(|_| {
                        format!(
                            "could not open media {} for upload",
                            object.source_path.display()
                        )
                    })?,
            )
            .send()
            .await
            .map_err(|_| format!("Wasabi rejected media object {}", object.object_key))?;
        Ok(())
    }
}

fn media_metadata_queries(
    connection: &Connection,
    object: &MediaObject,
) -> Result<Vec<D1Query>, String> {
    let parts = object.object_key.split('/').collect::<Vec<_>>();
    let kind_index = parts
        .iter()
        .position(|part| matches!(*part, "images" | "epubs" | "audio"))
        .ok_or_else(|| format!("media key has no known object kind: {}", object.object_key))?;
    let kind = parts[kind_index];
    let file_name = parts
        .last()
        .copied()
        .ok_or_else(|| format!("media key has no filename: {}", object.object_key))?;
    let (object_id, extension) = file_name
        .rsplit_once('.')
        .ok_or_else(|| format!("media key has no extension: {}", object.object_key))?;
    let (entity_id, original_name, cover_book_id) = match kind {
        "images" => {
            let book = connection
                .query_row(
                    "SELECT id, cover_url FROM books WHERE cover_url = ? LIMIT 1",
                    params![file_name],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(|_| "could not resolve a book cover reference".to_string())?;
            if let Some((id, name)) = book {
                (id, Some(name), Some(id))
            } else {
                let cd = connection
                    .query_row(
                        "SELECT id, cover_url FROM cds WHERE cover_url = ? LIMIT 1",
                        params![file_name],
                        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()
                    .map_err(|_| "could not resolve a CD cover reference".to_string())?
                    .ok_or_else(|| {
                        format!("media cover is not referenced by SQLite: {file_name}")
                    })?;
                (cd.0, Some(cd.1), None)
            }
        }
        "epubs" => connection
            .query_row(
                "SELECT id, epub_file_name FROM books
                 WHERE epub_file_hash = ? OR epub_file_hash = ?
                 LIMIT 1",
                params![object_id, file_name],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()
            .map_err(|_| "could not resolve an EPUB reference".to_string())?
            .map(|(id, name)| (id, name, None))
            .ok_or_else(|| format!("media EPUB is not referenced by SQLite: {file_name}"))?,
        "audio" => connection
            .query_row(
                "SELECT id, file_name FROM tracks
                 WHERE file_hash = ? OR file_hash = ? OR file_hash LIKE ?
                 LIMIT 1",
                params![object_id, file_name, format!("{object_id}.%")],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()
            .map_err(|_| "could not resolve an audio reference".to_string())?
            .map(|(id, name)| (id, name, None))
            .ok_or_else(|| format!("media audio is not referenced by SQLite: {file_name}"))?,
        _ => unreachable!(),
    };
    let object_kind = match kind {
        "images" => "cover",
        "epubs" => "epub",
        "audio" => "audio",
        _ => unreachable!(),
    };
    let mut queries = vec![D1Query {
        sql: "INSERT OR IGNORE INTO object_uploads
              (object_key, object_kind, entity_id, content_type, extension,
               expected_size, original_name, status, created_at)
              VALUES (?, ?, ?, ?, ?, ?, ?, 'complete', CURRENT_TIMESTAMP)"
            .to_string(),
        params: vec![
            json!(object.object_key),
            json!(object_kind),
            json!(entity_id),
            json!(object.content_type),
            json!(extension),
            json!(object.size_bytes),
            original_name.map_or(JsonValue::Null, JsonValue::from),
        ],
    }];
    if kind == "images" {
        queries.push(D1Query {
            sql: "INSERT OR IGNORE INTO cover_objects
                  (object_key, book_id, content_type, extension, expected_size,
                   content_sha3_256, status, created_at)
                  VALUES (?, ?, ?, ?, ?, ?, 'complete', CURRENT_TIMESTAMP)"
                .to_string(),
            params: vec![
                json!(object.object_key),
                cover_book_id.map_or(JsonValue::Null, JsonValue::from),
                json!(object.content_type),
                json!(extension),
                json!(object.size_bytes),
                json!(object.sha3_256),
            ],
        });
    }
    Ok(queries)
}

async fn reconcile(connection: &Connection, d1: &D1Client) -> Result<ReconciliationReport, String> {
    let mut source_counts = BTreeMap::new();
    let mut destination_counts = BTreeMap::new();
    let mut missing_counts = BTreeMap::new();
    let mut extra_counts = BTreeMap::new();
    for table in TABLES {
        let source_count = read_table(connection, table)?.len();
        let destination_count = d1.count(table).await?;
        source_counts.insert((*table).to_string(), source_count);
        destination_counts.insert((*table).to_string(), destination_count);
        missing_counts.insert(
            (*table).to_string(),
            source_count.saturating_sub(destination_count),
        );
        extra_counts.insert(
            (*table).to_string(),
            destination_count.saturating_sub(source_count),
        );
    }
    Ok(ReconciliationReport {
        source_counts,
        destination_counts,
        missing_counts,
        extra_counts,
    })
}

fn load_state(path: &Path) -> Result<MigrationState, String> {
    if !path.exists() {
        return Ok(MigrationState::default());
    }
    let bytes = fs::read(path).map_err(|_| "could not read migration state".to_string())?;
    serde_json::from_slice(&bytes).map_err(|_| "migration state is invalid JSON".to_string())
}

fn save_state(path: &Path, state: &MigrationState) -> Result<(), String> {
    write_json(path, state)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| "could not encode migration JSON".to_string())?;
    fs::write(&temporary, bytes).map_err(|_| "could not write migration JSON".to_string())?;
    fs::rename(&temporary, path).map_err(|_| "could not commit migration JSON".to_string())
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} is required for migration apply"))
}
fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_arguments_are_explicit() {
        let args = Args::parse(
            [
                "--sqlite",
                "source.db",
                "--media-root",
                "data",
                "--report",
                "report.json",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .expect("valid migration arguments");
        assert!(!args.apply);
        assert_eq!(args.sqlite, PathBuf::from("source.db"));
        assert_eq!(args.media_root, PathBuf::from("data"));
    }

    #[test]
    fn destination_rows_strip_audio_hash_extensions() {
        let row = SqlRow {
            columns: vec![
                "id".to_string(),
                "file_hash".to_string(),
                "file_name".to_string(),
            ],
            values: vec![
                Value::Integer(1),
                Value::Text("audiohash.mp3".to_string()),
                Value::Text("track.mp3".to_string()),
            ],
        };
        let destination = row.destination_row("tracks");
        assert_eq!(destination.values[1], Value::Text("audiohash".to_string()));
        assert_eq!(destination.values[2], Value::Text("track.mp3".to_string()));
    }
    #[test]
    fn blob_values_are_encoded_as_sqlite_literals() {
        let row = SqlRow {
            columns: vec!["id".to_string(), "blob".to_string()],
            values: vec![Value::Integer(1), Value::Blob(vec![0, 255])],
        };
        let query = row.insert_query("example").expect("insert query");
        assert!(query.sql.contains("X'00ff'"));
        assert_eq!(query.params, vec![json!(1)]);
    }

    #[test]
    fn destination_rows_use_worker_media_filenames() {
        let row = SqlRow {
            columns: vec![
                "id".to_string(),
                "epub_file_hash".to_string(),
                "epub_file_name".to_string(),
            ],
            values: vec![
                Value::Integer(7),
                Value::Text("abc123".to_string()),
                Value::Text("book.epub".to_string()),
            ],
        };
        let destination = row.destination_row("books");
        assert_eq!(
            destination.values[1],
            Value::Text("abc123.epub".to_string())
        );
    }

    #[test]
    fn object_keys_reject_traversal_and_accept_prefixes() {
        assert!(object_key(None, "images/../secret.jpg").is_err());
        assert_eq!(
            object_key(Some("migration/test"), "audio/hash.mp3").unwrap(),
            "migration/test/audio/hash.mp3"
        );
    }

    #[tokio::test]
    async fn dry_run_fixture_covers_rows_and_media() {
        let suffix = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        );
        let root = env::temp_dir().join(format!("dantalian-migration-{suffix}"));
        let database = root.join("source.db");
        let media_root = root.join("data");
        let state = root.join("state.json");
        let report = root.join("report.json");
        fs::create_dir_all(media_root.join("audio/encoded/opus"))
            .expect("fixture audio directories");
        fs::create_dir_all(media_root.join("images")).expect("fixture image directory");
        fs::write(media_root.join("images/cover.jpg"), b"cover fixture").expect("fixture media");
        fs::write(media_root.join("audio/audiohash.mp3"), b"audio fixture").expect("fixture audio");
        fs::write(
            media_root.join("audio/encoded/opus/audiohash.opus"),
            b"encoded fixture",
        )
        .expect("fixture encoded audio");
        let connection = Connection::open(&database).expect("fixture database");
        for table in TABLES {
            let sql = match *table {
                "series" => "CREATE TABLE series (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
                "books" => "CREATE TABLE books (
                        id INTEGER PRIMARY KEY,
                        cover_url TEXT,
                        epub_file_hash TEXT,
                        epub_file_name TEXT
                    )"
                .to_string(),
                "cds" => "CREATE TABLE cds (id INTEGER PRIMARY KEY, cover_url TEXT)".to_string(),
                "tracks" => "CREATE TABLE tracks (
                        id INTEGER PRIMARY KEY,
                        file_hash TEXT,
                        file_name TEXT
                    )"
                .to_string(),
                _ => format!("CREATE TABLE \"{table}\" (id INTEGER PRIMARY KEY)"),
            };
            connection.execute(&sql, []).expect("fixture table");
        }
        connection
            .execute("INSERT INTO series (id, name) VALUES (1, 'fixture')", [])
            .expect("fixture row");
        connection
            .execute(
                "INSERT INTO books (id, cover_url) VALUES (1, 'cover.jpg')",
                [],
            )
            .expect("fixture book");
        connection
            .execute(
                "INSERT INTO tracks (id, file_hash, file_name)
                 VALUES (1, 'audiohash.mp3', 'track.mp3')",
                [],
            )
            .expect("fixture track");
        drop(connection);

        run(Args {
            sqlite: database.clone(),
            media_root,
            apply: false,
            state_path: state.clone(),
            report_path: report.clone(),
        })
        .await
        .expect("dry-run migration");
        let report_json: JsonValue =
            serde_json::from_slice(&fs::read(&report).expect("report file")).expect("report JSON");
        assert_eq!(report_json["mode"], "dry-run");
        assert_eq!(report_json["tables"]["series"]["source_rows"], 1);
        assert_eq!(
            report_json["media"].as_array().expect("media array").len(),
            3
        );
        assert!(!state.exists());
        fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[test]
    fn dotted_epub_hash_is_not_double_suffixed() {
        assert_eq!(normalized_media_name("abc123.epub", "epub"), "abc123.epub");
        assert_eq!(normalized_media_name("abc123", "epub"), "abc123.epub");
    }
}
